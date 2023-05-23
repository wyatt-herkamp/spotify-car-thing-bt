use crate::error::AppError;
use crate::sys::{BtSocketListener, BtSocketStream, Platform};
use crate::workers::deskthing_bridge::spawn_deskthing_bridge_workers;
use crate::workers::deskthing_bridge::DeskthingChans;
use crate::workers::json_websocket::spawn_json_websocket_workers;
use crate::workers::stock_spotify::spawn_car_thing_workers;
use crate::workers::stock_spotify::CarThingServerChans;
use log::{debug, error, info};
use std::net::TcpListener;
use uuid::Uuid;

pub const GUID_SPOTIFY: Uuid = Uuid::from_fields(
    0xe3cccccd,
    0x33b7,
    0x457d,
    &[0xa0, 0x3c, 0xaa, 0x1c, 0x54, 0xbf, 0x61, 0x7f],
);

fn accept_car_thing<P: Platform+'static>(chans: DeskthingChans) -> Result<(), AppError> {
    let mut bt_socket = P::bind_bt_socket_listener()?;
    bt_socket.register_service("Spotify Car Thing", GUID_SPOTIFY)?;

    loop {
        let bt_sock = {
            info!(
                "waiting for bt connection on RFCOMM port {}...",
                bt_socket.rfcomm_port()
            );
            let bt_sock = bt_socket.accept()?;
            debug!(
                "Connection received on port {}",
                bt_sock.port()
            );
            bt_sock
        };

        let (
            car_thing_server,
            CarThingServerChans {
                topic_tx,
                rpc_req_rx,
                rpc_res_tx,
                state_req_rx,
            },
        ) = spawn_car_thing_workers(Box::new(bt_sock.try_clone()?), Box::new(bt_sock))?;

        chans
            .update_bt(topic_tx, state_req_rx, rpc_req_rx, rpc_res_tx)?;

        car_thing_server.wait_for_shutdown()
    }
}

fn accept_websocket(chans: DeskthingChans) -> Result<(), AppError> {
    let port = super::get_deskthing_port();
    let ws_server = TcpListener::bind(format!("127.0.0.1:{port}"))?;
    loop {
        let ws_stream = {
            info!("waiting for ws connection on 127.0.0.1:{port}...");
            let (ws_stream, ws_addr) = ws_server.accept()?;
            info!("accepted ws connection from {}", ws_addr);
            ws_stream
        };

        let (ws_server, ws_tx, ws_rx) =
            spawn_json_websocket_workers(ws_stream)?;

        chans.update_ws(ws_tx, ws_rx)?;

        if ws_server.wait_for_shutdown().is_err() {
            error!("car_thing_server did not shut down cleanly")
        }
    }
}

pub fn run_deskthing<P: Platform+'static>() -> Result<(), AppError> {
    let (deskthing_server, chans) = spawn_deskthing_bridge_workers()?;

    let _accept_car_thing = {
        let chans = chans.clone();
        std::thread::spawn(move || {
            if let Err(e) = accept_car_thing::<P>(chans) {
                error!("failure accepting bt connection: {:?}", e)
            }
        })
    };

    let _accept_websocket = {
        let chans = chans;
        std::thread::spawn(move || {
            if let Err(e) = accept_websocket(chans) {
                error!("failure accepting ws connection: {:?}", e)
            }
        })
    };

    if deskthing_server.wait_for_shutdown().is_err() {
        error!("deskthing_server did not shut down cleanly")
    }

    Ok(())
}
