#[macro_use]
extern crate actix;
extern crate actix_web;
extern crate actix_broker;
extern crate futures;

use actix::{fut, ActorContext, AsyncContext, SystemService, WrapFuture, ActorFuture, ContextFutureSpawner};
use actix_web::Error as ActixWebError;
use actix_web::{ws, App, HttpRequest, HttpResponse, server::HttpServer};
use actix_broker::{BrokerIssue, BrokerSubscribe};

use std::collections::HashSet;

type Client = actix::Recipient<ChatMessage>;

struct Ws;
#[derive(Clone, Message)]
struct ChatMessage(String);
#[derive(Clone, Message)]
struct SendMessage(String);
#[derive(Clone, Message)]
struct Register(Client);


#[derive(Default)]
struct WsChatServer {
    clients: HashSet<Client>,
}

impl Ws {
    fn send_message(&self, msg: &str) {
        let msg = SendMessage(format!("{}", msg));
        self.issue_async(msg)
    }
}

impl WsChatServer {
    fn register_client(&mut self, client: Client) -> Option<()> {
        self.clients.insert(client);
        Some(())
    }

    fn send_chat_message(&mut self, msg: &str) -> Option<()> {
        for client in &self.clients {
            client.do_send(ChatMessage(msg.to_owned())).expect("Tried to send message to actor");
        }
        Some(())
    }
}

impl actix::Actor for Ws {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let register_msg = Register(
            ctx.address().recipient()
        );
        WsChatServer::from_registry()
            .send(register_msg)
            .into_actor(self)
            .then(|_,_,_| {fut::ok(())}).spawn(ctx);
    }
}

impl actix::Actor for WsChatServer {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.subscribe_async::<SendMessage>(ctx);
    }
}

impl actix::StreamHandler<ws::Message, ws::ProtocolError> for Ws {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Text(text) => {
                let content = text.trim();
                self.send_message(content);
            }
            ws::Message::Close(_) => {ctx.stop();}
            _ => {}
        }
    }
}

impl actix::Handler<ChatMessage> for Ws {
    type Result = ();

    fn handle(&mut self, msg: ChatMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl actix::Handler<SendMessage> for WsChatServer {
    type Result = ();

    fn handle(&mut self, msg: SendMessage, _ctx: &mut Self::Context) {
        let SendMessage(content) = msg;
        self.send_chat_message(&content);
    }
}

impl actix::Handler<Register> for WsChatServer {
    type Result = ();

    fn handle(&mut self, msg: Register, _ctx: &mut Self::Context) {
        let Register(client) = msg;
        self.register_client(client);
    }
}

impl actix::SystemService for WsChatServer {}
impl actix::Supervised for WsChatServer {}

fn ws_route(req: &HttpRequest<()>) -> Result<HttpResponse, ActixWebError> {
    ws::start(req, Ws)
}

fn main() {
    let sys = actix::System::new("websocket-load-test");

    HttpServer::new(move || {
        App::new()
            .resource("/ws", |r| r.route().f(ws_route))
    }).bind("127.0.0.1:8080")
        .unwrap()
        .start();
    let _ = sys.run();
}
