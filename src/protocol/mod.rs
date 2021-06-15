use crate::bootstrap::Connection;
use crate::client::instance::{Client, ClientInfo, State};
use crate::error::Res;
use crate::client::runner::GlobalState;

pub mod v340;

mod types;
mod io;
mod transform;
mod encrypt;

#[async_trait::async_trait]
pub trait McProtocol where Self: Sized {
    async fn login(conn: Connection) -> Res<Login<Self>>;
    fn apply_packets(&mut self, client: &mut State, global: &mut GlobalState);
    fn send_chat(&mut self, message: &str);
    fn teleport(&mut self);
    fn disconnected(&self) -> bool;
}


pub struct Login<T: McProtocol> {
    pub protocol: T,
    pub info: ClientInfo
}
