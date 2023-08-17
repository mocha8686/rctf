use std::io::Read;

use anyhow::Result;
use ssh2::Session;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<()> {
    let tcp = TcpStream::connect(("pwnable.kr", 2222)).await?;
    let mut session = Session::new()?;
    session.set_tcp_stream(tcp);
    session.handshake()?;
    session.userauth_password("fd", "guest")?;

    let mut channel = session.channel_session()?;
    channel.exec("ls -l")?;
    let mut s = String::new();
    channel.read_to_string(&mut s)?;
    println!("{}", s);
    channel.wait_close()?;
    println!("Exit code: {}", channel.exit_status()?);

    Ok(())
}
