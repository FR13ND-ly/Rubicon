use std::{
    collections::HashMap,
    future::{Ready, ready},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use deno_core::futures::future::Either;
use tower_http::services::fs::{Backend, TokioBackend};

/// The asset layout is a startup snapshot; existing file contents remain live.
/// Unknown or unreadable subtrees and symlinks retain the filesystem backend.
#[derive(Clone)]
pub(crate) struct IndexedBackend<B = TokioBackend> {
    index: Arc<AssetIndex>,
    backend: B,
}

struct AssetIndex {
    root: PathBuf,
    entries: HashMap<PathBuf, Option<std::fs::Metadata>>,
    uncached: Vec<PathBuf>,
}

impl AssetIndex {
    fn build(root: PathBuf) -> Self {
        // Match ServeDir's treatment of relative paths.
        let mut base = PathBuf::from(".");
        base.push(root);
        let mut index = Self {
            root: base.clone(),
            entries: HashMap::new(),
            uncached: Vec::new(),
        };
        let mut directories = vec![base];
        while let Some(directory) = directories.pop() {
            let key = index
                .key(&directory)
                .expect("Indexed directory is below root");
            let metadata = match std::fs::metadata(&directory) {
                Ok(value) if value.is_dir() => value,
                _ => {
                    index.uncached.push(key);
                    continue;
                }
            };
            index.entries.insert(key.clone(), Some(metadata));
            let children = match std::fs::read_dir(&directory) {
                Ok(value) => value,
                Err(_) => {
                    index.uncached.push(key);
                    continue;
                }
            };
            for child in children {
                let Ok(child) = child else {
                    index.uncached.push(key.clone());
                    continue;
                };
                let path = child.path();
                let Some(child_key) = index.key(&path) else {
                    // Windows Unicode case matching is delegated to the filesystem.
                    index.uncached.push(key.clone());
                    continue;
                };
                match child.file_type() {
                    Ok(kind) if kind.is_symlink() => index.uncached.push(child_key),
                    Ok(kind) if kind.is_dir() => directories.push(path),
                    Ok(_) => {
                        index.entries.insert(child_key, None);
                    }
                    Err(_) => index.uncached.push(child_key),
                }
            }
        }
        index
    }

    fn key(&self, path: &Path) -> Option<PathBuf> {
        let relative = path.strip_prefix(&self.root).ok()?;
        #[cfg(windows)]
        {
            let text = relative.to_str()?;
            if !text.is_ascii() || text.contains([':', '~']) {
                return None;
            }
            let folded = text.to_ascii_lowercase();
            for component in folded.split(['/', '\\']) {
                let stem = component.split('.').next().unwrap_or("");
                if component.ends_with(['.', ' '])
                    || matches!(stem, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$")
                    || ((stem.starts_with("com") || stem.starts_with("lpt"))
                        && stem.len() == 4
                        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
                {
                    return None;
                }
            }
            Some(PathBuf::from(folded))
        }
        #[cfg(not(windows))]
        Some(relative.to_owned())
    }

    fn lookup(&self, path: &Path) -> Lookup<'_> {
        let Some(key) = self.key(path) else {
            return Lookup::Filesystem;
        };
        if self.uncached.iter().any(|prefix| key.starts_with(prefix)) {
            return Lookup::Filesystem;
        }
        match self.entries.get(&key) {
            Some(Some(metadata)) => Lookup::Directory(metadata),
            Some(None) => Lookup::Filesystem,
            None => {
                // Preserve filesystem errors for paths descending through a file.
                if key
                    .ancestors()
                    .skip(1)
                    .any(|parent| matches!(self.entries.get(parent), Some(None)))
                {
                    Lookup::Filesystem
                } else {
                    Lookup::Missing
                }
            }
        }
    }
}

enum Lookup<'a> {
    Directory(&'a std::fs::Metadata),
    Filesystem,
    Missing,
}

impl IndexedBackend {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            index: Arc::new(AssetIndex::build(root)),
            backend: TokioBackend,
        }
    }
}

impl<B: Backend<Metadata = std::fs::Metadata>> Backend for IndexedBackend<B> {
    type File = B::File;
    type Metadata = std::fs::Metadata;
    type OpenFuture = Either<Ready<io::Result<Self::File>>, B::OpenFuture>;
    type MetadataFuture = Either<Ready<io::Result<Self::Metadata>>, B::MetadataFuture>;

    fn open(&self, path: PathBuf) -> Self::OpenFuture {
        match self.index.lookup(&path) {
            Lookup::Missing => Either::Left(ready(Err(io::ErrorKind::NotFound.into()))),
            _ => Either::Right(self.backend.open(path)),
        }
    }

    fn metadata(&self, path: PathBuf) -> Self::MetadataFuture {
        match self.index.lookup(&path) {
            Lookup::Directory(metadata) => Either::Left(ready(Ok(metadata.clone()))),
            Lookup::Missing => Either::Left(ready(Err(io::ErrorKind::NotFound.into()))),
            Lookup::Filesystem => Either::Right(self.backend.metadata(path)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::Request,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower_http::services::ServeDir;

    #[derive(Clone, Default)]
    struct CountedFilesystem {
        calls: Arc<AtomicUsize>,
    }

    impl Backend for CountedFilesystem {
        type File = <TokioBackend as Backend>::File;
        type Metadata = std::fs::Metadata;
        type OpenFuture = <TokioBackend as Backend>::OpenFuture;
        type MetadataFuture = <TokioBackend as Backend>::MetadataFuture;

        fn open(&self, path: PathBuf) -> Self::OpenFuture {
            self.calls.fetch_add(1, Ordering::Relaxed);
            TokioBackend.open(path)
        }

        fn metadata(&self, path: PathBuf) -> Self::MetadataFuture {
            self.calls.fetch_add(1, Ordering::Relaxed);
            TokioBackend.metadata(path)
        }
    }

    struct Assets(PathBuf);

    impl Assets {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("rubicon-index-{}-{unique}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("file.css"), "initial").unwrap();
            Self(path)
        }

        fn backend(&self) -> IndexedBackend<CountedFilesystem> {
            IndexedBackend {
                index: Arc::new(AssetIndex::build(self.0.clone())),
                backend: CountedFilesystem::default(),
            }
        }
    }

    impl Drop for Assets {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("file.css"));
            let _ = std::fs::remove_file(self.0.join("later.css"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[tokio::test]
    async fn ssr_misses_and_directory_root_do_not_touch_the_filesystem() {
        let assets = Assets::new();
        let backend = assets.backend();
        let mut service = ServeDir::with_backend(&assets.0, backend.clone())
            .append_index_html_on_directories(false)
            .fallback(Router::new().fallback(|| async { "SSR" }));
        for path in ["/", "/products?sort=asc", "/nested/missing", "/%70roducts"] {
            for method in ["GET", "HEAD"] {
                let response = service
                    .try_call(
                        Request::builder()
                            .method(method)
                            .uri(path)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), 200);
            }
        }
        assert_eq!(backend.backend.calls.load(Ordering::Relaxed), 0);
        let response = service
            .try_call(
                Request::builder()
                    .uri("/%66ile.css?version=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            to_bytes(Body::new(response.into_body()), 1024)
                .await
                .unwrap(),
            "initial"
        );
        assert!(backend.backend.calls.load(Ordering::Relaxed) > 0);
    }

    #[tokio::test]
    async fn indexed_file_contents_stay_live_and_new_names_require_reindexing() {
        let assets = Assets::new();
        let backend = assets.backend();
        std::fs::write(assets.0.join("file.css"), "updated").unwrap();
        std::fs::write(assets.0.join("later.css"), "new file").unwrap();
        assert!(matches!(
            backend.index.lookup(&assets.0.join("later.css")),
            Lookup::Missing
        ));
        assert!(matches!(
            backend.index.lookup(&assets.0.join("file.css/child")),
            Lookup::Filesystem
        ));
        #[cfg(windows)]
        for name in [
            "FILE.CSS",
            "file.css.",
            "file.css ",
            "file.css::$DATA",
            "FILE~1.CSS",
            "NUL",
            "COM1.txt",
        ] {
            assert!(matches!(
                backend.index.lookup(&assets.0.join(name)),
                Lookup::Filesystem
            ));
        }
        let mut service = ServeDir::with_backend(&assets.0, backend.clone());
        let response = service
            .try_call(
                Request::builder()
                    .uri("/file.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            to_bytes(Body::new(response.into_body()), 1024)
                .await
                .unwrap(),
            "updated"
        );
        let refreshed = assets.backend();
        assert!(matches!(
            refreshed.index.lookup(&assets.0.join("later.css")),
            Lookup::Filesystem
        ));
        std::fs::remove_file(assets.0.join("file.css")).unwrap();
        assert!(
            matches!(service.try_call(Request::builder().uri("/file.css")
            .body(Body::empty()).unwrap()).await,
            Err(error) if error.kind() == io::ErrorKind::NotFound)
        );
    }

    #[cfg(windows)]
    #[test]
    fn unicode_names_leave_parent_case_matching_to_windows() {
        let assets = Assets::new();
        let unicode = assets.0.join("țară.css");
        std::fs::write(&unicode, "unicode").unwrap();
        let backend = assets.backend();
        std::fs::remove_file(unicode).unwrap();
        assert!(matches!(
            backend.index.lookup(&assets.0.join("missing")),
            Lookup::Filesystem
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_directories_remain_live_without_recursive_indexing() {
        let assets = Assets::new();
        let link = assets.0.join("linked");
        std::os::unix::fs::symlink(&assets.0, &link).unwrap();
        let backend = assets.backend();
        std::fs::remove_file(&link).unwrap();
        assert!(matches!(
            backend.index.lookup(&link.join("file.css")),
            Lookup::Filesystem
        ));
    }
}
