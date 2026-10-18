use alloc::boxed::Box;
use bt_hci::{
    ControllerToHostPacket,
    cmd::{
        SyncCmd,
        controller_baseband::{Reset, SetEventMask},
        le::{LeSetEventMask, LeSetScanEnable, LeSetScanParams},
    },
    controller::{Controller, ExternalController},
    event::EventKind,
    param::{AddrKind, Duration, EventMask, LeEventMask, LeScanKind, ScanningFilterPolicy},
};
use embassy_futures::{
    select::{Either, select},
    yield_now,
};
use embassy_time::Timer;
use esp_hal::peripherals::BT;
use esp_radio::ble::controller::BleConnector;
use paper_mesh_platform::discovery::{ScanError, ScanRequest};

type BleController<'a> = ExternalController<BleConnector<'a>, 1>;

pub fn scan(device: BT<'_>, request: ScanRequest) -> super::ScanFuture<'_> {
    Box::pin(run(device, request))
}

async fn run(device: BT<'_>, request: ScanRequest) -> Result<(), ScanError> {
    let connector = BleConnector::new(device, Default::default()).map_err(failed)?;
    let controller = ExternalController::<_, 1>::new(connector);
    // The reader must process command acknowledgments while the command future waits.
    match select(commands(&controller), receive(&controller, request)).await {
        Either::First(result) | Either::Second(result) => result,
    }
}

fn commands<'a>(controller: &'a BleController<'_>) -> super::ScanFuture<'a> {
    Box::pin(run_commands(controller))
}

async fn run_commands(controller: &BleController<'_>) -> Result<(), ScanError> {
    execute(controller, Reset::new()).await?;
    execute(
        controller,
        SetEventMask::new(
            EventMask::new()
                .enable_le_meta(true)
                .enable_hardware_error(true),
        ),
    )
    .await?;
    execute(
        controller,
        LeSetEventMask::new(LeEventMask::new().enable_le_adv_report(true)),
    )
    .await?;
    execute(
        controller,
        LeSetScanParams::new(
            LeScanKind::Passive,
            Duration::from_millis(100),
            Duration::from_millis(100),
            AddrKind::PUBLIC,
            ScanningFilterPolicy::BasicUnfiltered,
        ),
    )
    .await?;
    execute(controller, LeSetScanEnable::new(true, false)).await?;
    Timer::after_secs(4).await;
    execute(controller, LeSetScanEnable::new(false, false)).await?;
    Ok(())
}

// Command futures use the radio heap to keep temporary storage below the stack limit.
fn execute<'a, C: SyncCmd<Return = ()> + 'a>(
    controller: &'a BleController<'_>,
    command: C,
) -> super::ScanFuture<'a> {
    Box::pin(async move { command.exec(controller).await.map_err(failed) })
}

fn receive<'a>(controller: &'a BleController<'_>, request: ScanRequest) -> super::ScanFuture<'a> {
    Box::pin(receive_events(controller, request))
}

async fn receive_events(
    controller: &BleController<'_>,
    request: ScanRequest,
) -> Result<(), ScanError> {
    // The transport includes one indicator byte, two header bytes, and up to 255 payload bytes.
    let mut buffer = [0; 258];
    loop {
        if let ControllerToHostPacket::Event(event) =
            controller.read(&mut buffer).await.map_err(failed)?
        {
            match event.kind {
                EventKind::Le => super::advertisement::visit(event.data, |network| {
                    super::observe(request, network);
                })?,
                EventKind::HardwareError => return Err(ScanError::Failed),
                _ => {}
            }
        }
        yield_now().await;
    }
}

fn failed(error: impl core::fmt::Debug) -> ScanError {
    log::warn!("Bluetooth LE scan failed: {:?}", error);
    ScanError::Failed
}
