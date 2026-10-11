use deno_core::op2;

use crate::catalog::{self, Product};

#[op2]
#[serde]
async fn op_demo_list_products() -> Vec<Product> {
    catalog::list_products().await
}

deno_core::extension!(
    rubicon_demo_catalog,
    ops = [op_demo_list_products],
    esm_entry_point = "ext:rubicon_demo_catalog/demo_catalog.js",
    esm = [dir "js", "demo_catalog.js"],
);

pub(crate) fn extension() -> deno_core::Extension {
    rubicon_demo_catalog::init()
}
