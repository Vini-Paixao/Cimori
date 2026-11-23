//! Reads the AIs' limits once with this machine's sign-ins and prints them:
//! `cargo run -p isle-ai --example usage`

#[tokio::main(flavor = "current_thread")]
async fn main() {
    isle_ai::usage::run(false, |providers| {
        println!("{}", serde_json::to_string_pretty(&providers).unwrap_or_default());
        std::process::exit(0);
    })
    .await;
}
