//! Conversion of legacy Bluetooth LE advertisements into shared observations.

use bt_hci::{
    FromHciBytes,
    event::le::{LeAdvertisingReport, LeEventKind, LeEventPacket},
    param::AddrKind,
};
use paper_mesh_app::platform::discovery::{AddressKind, Network, ScanError};

pub fn visit(data: &[u8], mut observe: impl FnMut(Network)) -> Result<(), ScanError> {
    let event = LeEventPacket::from_hci_bytes_complete(data).map_err(|_| ScanError::Failed)?;
    if event.kind != LeEventKind::LeAdvertisingReport {
        return Ok(());
    }
    let reports =
        LeAdvertisingReport::from_hci_bytes_complete(event.data).map_err(|_| ScanError::Failed)?;
    for report in reports.reports.iter() {
        let report = report.map_err(|_| ScanError::Failed)?;
        let address_kind = match report.addr_kind {
            AddrKind::PUBLIC => AddressKind::Public,
            AddrKind::RANDOM => AddressKind::Random,
            _ => return Err(ScanError::Failed),
        };
        let mut address = [0; 6];
        address.copy_from_slice(report.addr.raw());
        address.reverse();
        let rssi = match report.rssi {
            127 => None,
            -127..=20 => Some(i16::from(report.rssi)),
            _ => return Err(ScanError::Failed),
        };
        observe(Network::new(
            local_name(report.data).unwrap_or_default(),
            address,
            address_kind,
            rssi,
        ));
    }
    Ok(())
}

fn local_name(mut data: &[u8]) -> Result<&[u8], ScanError> {
    let mut short = &[][..];
    let mut complete = None;
    while let Some((&length, remaining)) = data.split_first() {
        if length == 0 {
            break;
        }
        let length = usize::from(length);
        let field = remaining.get(..length).ok_or(ScanError::Failed)?;
        match field[0] {
            0x08 => short = &field[1..],
            0x09 => complete = Some(&field[1..]),
            _ => {}
        }
        data = &remaining[length..];
    }
    Ok(complete.unwrap_or(short))
}
