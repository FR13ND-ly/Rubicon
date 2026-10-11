use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Product {
    pub id: String,
    pub name: String,
}

pub(crate) async fn list_products() -> Vec<Product> {
    vec![
        Product {
            id: "keyboard".to_owned(),
            name: "Tastatură".to_owned(),
        },
        Product {
            id: "mouse".to_owned(),
            name: "Mouse".to_owned(),
        },
        Product {
            id: "monitor".to_owned(),
            name: "Monitor".to_owned(),
        },
    ]
}
