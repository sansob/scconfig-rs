use scconfig_rs::{ConfigDocument, ResourceRequest, SpringConfigClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SpringConfigClient::builder("http://localhost:8888")?
        .default_label("main")
        .build()?;

    let request = ResourceRequest::new("inventory-service", ["dev"], "nginx.conf")?;
    let resource = client.fetch_resource(&request).await?;

    match resource.parse()? {
        ConfigDocument::Text(text) => println!("{text}"),
        ConfigDocument::Binary(bytes) => println!("received {} bytes", bytes.len()),
        document => println!("parsed {:?}", document.format()),
    }

    Ok(())
}
