use openrgb2::OpenRgbClient;
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut c = OpenRgbClient::connect().await.expect("connect");
    println!("protocol {}", c.get_protocol_version());
    match c.get_all_controllers().await {
        Ok(g) => for d in g.iter() {
            let zs: Vec<String> = d.get_all_zones().map(|z| format!("{}:{}:{:?}", z.name(), z.num_leds(), z.matrix_data().map(|m| (m.width(), m.height())))).collect();
            println!("{} | {:?}", d.name(), zs);
        },
        Err(e) => println!("ERROR get_all_controllers: {e}"),
    }
}
