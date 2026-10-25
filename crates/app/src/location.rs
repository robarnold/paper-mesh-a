//! Shared location screen, observation validity, and display deadlines.

use crate::screen::STYLE;
use paper_mesh_platform::ui::TextRole;

use core::fmt::Write;
use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use heapless::String;
use paper_mesh_platform::{
    location::{
        Coordinates, Location, LocationAvailability, LocationSnapshot, SatelliteObservation,
        UtcTime,
    },
    time::ObservationWindow,
    ui::format::{
        coordinate_degrees as coordinate_label, course_true_degrees as course_label,
        elapsed_seconds as age_label, elevation_meters as elevation_label, satellite_label,
        speed_kmh as speed_label, utc_time as utc_label,
    },
};

pub const LOCATION_MAX_AGE_MS: u64 = 5_000;
pub const LOCATION_REFRESH_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixStatus {
    Starting,
    Unavailable,
    Fault,
    Acquiring,
    NoData,
    Valid,
    Stale,
}

impl FixStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Receiver startup",
            Self::Unavailable => "No receiver data",
            Self::Fault => "Receiver fault",
            Self::Acquiring => "No position fix",
            Self::NoData => "No fix report",
            Self::Valid => "Valid position fix",
            Self::Stale => "Data expired",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverStatus {
    Waiting,
    Active,
    Quiet,
    Fault,
}

impl ReceiverStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "No response yet",
            Self::Active => "Active",
            Self::Quiet => "No recent response",
            Self::Fault => "Input fault",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocationView {
    pub status: FixStatus,
    pub coordinates: Option<Coordinates>,
    pub elevation_mm: Option<i32>,
    pub speed_mm_s: Option<u32>,
    pub course_millidegrees: Option<u32>,
    pub utc: Option<UtcTime>,
    pub satellites_used: Option<u8>,
    pub age_seconds: Option<u64>,
    pub receiver_status: ReceiverStatus,
    pub response_age_seconds: Option<u64>,
    pub valid_messages: u32,
    pub satellites: Option<SatelliteObservation>,
    pub satellite_age_seconds: Option<u64>,
}

impl LocationView {
    fn from_snapshot(snapshot: LocationSnapshot, now_ms: u64) -> Self {
        let window = ObservationWindow::new(now_ms, LOCATION_MAX_AGE_MS);
        let age = snapshot
            .observation
            .and_then(|value| window.age_ms(value.observed_at_ms));
        let fresh = snapshot
            .observation
            .is_some_and(|value| window.contains(value.observed_at_ms));
        let status = match snapshot.availability {
            LocationAvailability::Starting => FixStatus::Starting,
            LocationAvailability::Unavailable => FixStatus::Unavailable,
            LocationAvailability::Fault => FixStatus::Fault,
            LocationAvailability::Ready if snapshot.observation.is_some() && !fresh => {
                FixStatus::Stale
            }
            LocationAvailability::Ready
                if snapshot
                    .observation
                    .is_some_and(|value| value.coordinates.is_some()) =>
            {
                FixStatus::Valid
            }
            LocationAvailability::Ready if snapshot.observation.is_none() => FixStatus::NoData,
            LocationAvailability::Ready => FixStatus::Acquiring,
        };
        let usable = fresh && snapshot.availability == LocationAvailability::Ready;
        let response_age = snapshot
            .activity
            .and_then(|activity| window.age_ms(activity.received_at_ms));
        let receiver_status = if snapshot.availability == LocationAvailability::Fault {
            ReceiverStatus::Fault
        } else if snapshot
            .activity
            .is_some_and(|value| window.contains(value.received_at_ms))
        {
            ReceiverStatus::Active
        } else if snapshot.activity.is_some() {
            ReceiverStatus::Quiet
        } else {
            ReceiverStatus::Waiting
        };
        let satellite_age = snapshot
            .satellites
            .and_then(|satellites| window.age_ms(satellites.observed_at_ms));
        let current =
            |time| snapshot.availability == LocationAvailability::Ready && window.contains(time);
        let motion = snapshot
            .motion
            .filter(|value| current(value.observed_at_ms));
        let speed_mm_s = motion.and_then(|value| value.speed_mm_s);
        Self {
            status,
            coordinates: snapshot
                .observation
                .filter(|_| usable)
                .and_then(|value| value.coordinates),
            elevation_mm: snapshot
                .observation
                .filter(|value| usable && value.coordinates.is_some())
                .and_then(|value| value.elevation_mm),
            speed_mm_s,
            course_millidegrees: motion
                .filter(|_| speed_mm_s.is_some_and(|speed| speed > 0))
                .and_then(|value| value.course_millidegrees)
                .filter(|course| *course < 360_000),
            utc: snapshot
                .utc
                .filter(|value| current(value.observed_at_ms))
                .map(|value| value.time),
            satellites_used: snapshot
                .observation
                .filter(|_| usable)
                .and_then(|value| value.satellites_used),
            age_seconds: age.map(|age| age / 1000),
            receiver_status,
            response_age_seconds: response_age.map(|age| age / 1000),
            valid_messages: snapshot
                .activity
                .map_or(0, |activity| activity.valid_messages),
            satellites: snapshot
                .satellites
                .filter(|value| current(value.observed_at_ms)),
            satellite_age_seconds: satellite_age.map(|age| age / 1000),
        }
    }
}

pub struct LocationScreen {
    open: bool,
    view: LocationView,
    next_update_ms: u64,
    expires_at_ms: Option<u64>,
}

impl Default for LocationScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl LocationScreen {
    pub const fn new() -> Self {
        Self {
            open: false,
            view: LocationView {
                status: FixStatus::Starting,
                coordinates: None,
                elevation_mm: None,
                speed_mm_s: None,
                course_millidegrees: None,
                utc: None,
                satellites_used: None,
                age_seconds: None,
                receiver_status: ReceiverStatus::Waiting,
                response_age_seconds: None,
                valid_messages: 0,
                satellites: None,
                satellite_age_seconds: None,
            },
            next_update_ms: 0,
            expires_at_ms: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn view(&self) -> LocationView {
        self.view
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open(&mut self) {
        self.open = true;
        self.next_update_ms = 0;
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        self.open.then_some(
            self.next_update_ms
                .min(self.expires_at_ms.unwrap_or(u64::MAX)),
        )
    }

    pub fn poll(&mut self, now_ms: u64, location: &mut impl Location) -> bool {
        let snapshot = location.snapshot(now_ms);
        let view = LocationView::from_snapshot(snapshot, now_ms);
        self.expires_at_ms = ObservationWindow::new(now_ms, LOCATION_MAX_AGE_MS).next_expiry_ms(
            [
                snapshot.observation.map(|value| value.observed_at_ms),
                snapshot.motion.map(|value| value.observed_at_ms),
                snapshot.utc.map(|value| value.observed_at_ms),
                snapshot.activity.map(|value| value.received_at_ms),
                snapshot.satellites.map(|value| value.observed_at_ms),
            ]
            .into_iter()
            .flatten(),
        );
        if !self.open
            || now_ms >= self.next_update_ms
            || view.status != self.view.status
            || view.receiver_status != self.view.receiver_status
            || view.satellites.is_some() != self.view.satellites.is_some()
            || view.elevation_mm.is_some() != self.view.elevation_mm.is_some()
            || view.speed_mm_s.is_some() != self.view.speed_mm_s.is_some()
            || view.course_millidegrees.is_some() != self.view.course_millidegrees.is_some()
            || view.utc.is_some() != self.view.utc.is_some()
        {
            let changed = view != self.view;
            self.view = view;
            self.next_update_ms = now_ms.saturating_add(LOCATION_REFRESH_MS);
            return self.open && changed;
        }
        false
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(STYLE.background)?;
        let black = STYLE.text(TextRole::Body);
        let muted = STYLE.text(TextRole::Secondary);
        let mut label = String::<64>::new();
        let _ = write!(label, "Receiver: {}", self.view.receiver_status.label());
        Text::new(&label, Point::new(28, 132), black).draw(display)?;
        label.clear();
        let _ = write!(
            label,
            "Messages: {}  Last: {} s",
            self.view.valid_messages,
            age_label(self.view.response_age_seconds)
        );
        Text::new(&label, Point::new(28, 160), muted).draw(display)?;
        Text::new(self.view.status.label(), Point::new(28, 198), black).draw(display)?;
        Text::new("Lat", Point::new(28, 232), muted).draw(display)?;
        Text::new("Lon", Point::new(28, 260), muted).draw(display)?;
        let latitude = coordinate_label(
            self.view
                .coordinates
                .map(Coordinates::latitude_microdegrees),
        );
        let longitude = coordinate_label(
            self.view
                .coordinates
                .map(Coordinates::longitude_microdegrees),
        );
        Text::new(&latitude, Point::new(100, 232), black).draw(display)?;
        Text::new(&longitude, Point::new(100, 260), black).draw(display)?;
        label.clear();
        let _ = write!(
            label,
            "Elevation: {} m (MSL)",
            elevation_label(self.view.elevation_mm)
        );
        Text::new(&label, Point::new(28, 288), black).draw(display)?;
        label.clear();
        let _ = write!(label, "Speed: {} km/h", speed_label(self.view.speed_mm_s));
        Text::new(&label, Point::new(28, 322), black).draw(display)?;
        label.clear();
        let _ = write!(
            label,
            "Course: {}",
            course_label(self.view.course_millidegrees)
        );
        Text::new(&label, Point::new(28, 350), black).draw(display)?;
        label.clear();
        let _ = write!(label, "UTC: {}", utc_label(self.view.utc));
        Text::new(&label, Point::new(28, 378), black).draw(display)?;
        label.clear();
        let _ = write!(
            label,
            "WGS84 degrees  Fix age: {} s",
            age_label(self.view.age_seconds)
        );
        Text::new(&label, Point::new(28, 414), muted).draw(display)?;
        label.clear();
        if let Some(count) = self.view.satellites_used {
            let _ = write!(label, "Used: {count}");
        } else {
            let _ = label.push_str("Used: --");
        }
        if let Some(satellites) = self.view.satellites {
            let suffix = if satellites.truncated { "+" } else { "" };
            let _ = write!(
                label,
                "   In view: {}{suffix}",
                satellites.satellites_in_view
            );
        } else {
            let _ = label.push_str("   In view: --");
        }
        Text::new(&label, Point::new(28, 450), black).draw(display)?;
        label.clear();
        if let Some(satellites) = self.view.satellites {
            let suffix = if satellites.truncated { "+" } else { "" };
            let _ = write!(
                label,
                "With signal: {}{suffix}",
                satellites.satellites_with_signal
            );
        } else {
            let _ = label.push_str("With signal: --");
        }
        Text::new(&label, Point::new(28, 478), black).draw(display)?;
        Text::new("Signal C/N0 (dB-Hz)", Point::new(28, 514), muted).draw(display)?;
        Text::new("Satellite", Point::new(28, 542), muted).draw(display)?;
        Text::new("0", Point::new(190, 542), muted).draw(display)?;
        Text::new("50", Point::new(285, 542), muted).draw(display)?;
        Text::new("99", Point::new(380, 542), muted).draw(display)?;
        if let Some(satellites) = self.view.satellites {
            for (row, satellite) in satellites.strongest.iter().flatten().enumerate() {
                let y = 574 + row as i32 * 32;
                label.clear();
                let _ = write!(
                    label,
                    "{} {:03}",
                    satellite_label(satellite.system),
                    satellite.id
                );
                Text::new(&label, Point::new(28, y), black).draw(display)?;
                Rectangle::new(Point::new(190, y - 16), Size::new(210, 18))
                    .into_styled(PrimitiveStyle::with_stroke(STYLE.divider, 1))
                    .draw(display)?;
                label.clear();
                if let Some(level) = satellite.cn0_db_hz {
                    Rectangle::new(
                        Point::new(192, y - 14),
                        Size::new(u32::from(level.min(99)) * 206 / 99, 14),
                    )
                    .into_styled(PrimitiveStyle::with_fill(STYLE.foreground))
                    .draw(display)?;
                    let _ = write!(label, "{level}");
                } else {
                    let _ = label.push_str("--");
                }
                Text::new(&label, Point::new(424, y), black).draw(display)?;
            }
            if satellites.satellites_in_view == 0 {
                Text::new("No satellites reported.", Point::new(28, 576), black).draw(display)?;
            }
        } else {
            Text::new("No current satellite report.", Point::new(28, 576), black).draw(display)?;
        }
        label.clear();
        let _ = write!(
            label,
            "Satellite age: {} s",
            age_label(self.view.satellite_age_seconds)
        );
        Text::new(&label, Point::new(28, 840), muted).draw(display)?;
        Text::new(
            "Up to 8 satellites, strongest first.",
            Point::new(28, 872),
            muted,
        )
        .draw(display)?;
        Text::new(
            "-- means unavailable or expired.",
            Point::new(28, 900),
            muted,
        )
        .draw(display)?;
        Text::new("Readouts update every 5 s.", Point::new(28, 932), muted).draw(display)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FixStatus, LocationView, ReceiverStatus};
    use paper_mesh_platform::location::{
        Coordinates, LocationAvailability, LocationObservation, LocationSnapshot, MAX_SATELLITES,
        MotionObservation, ReceiverActivity, SatelliteObservation, UtcObservation, UtcTime,
    };

    #[test]
    fn future_observations_and_unavailable_providers_cannot_supply_a_fix() {
        let mut snapshot = LocationSnapshot {
            availability: LocationAvailability::Ready,
            observation: Some(LocationObservation {
                observed_at_ms: 1_000,
                coordinates: Coordinates::new(0, 0),
                elevation_mm: Some(0),
                satellites_used: None,
            }),
            motion: Some(MotionObservation {
                observed_at_ms: 1_000,
                speed_mm_s: Some(1_000),
                course_millidegrees: Some(0),
            }),
            utc: Some(UtcObservation {
                observed_at_ms: 1_000,
                time: UtcTime::new(0, 0, 0).unwrap(),
                date: None,
            }),
            activity: Some(ReceiverActivity {
                received_at_ms: 1_000,
                valid_messages: 5,
            }),
            satellites: Some(SatelliteObservation {
                observed_at_ms: 1_000,
                satellites_in_view: 0,
                satellites_with_signal: 0,
                strongest: [None; MAX_SATELLITES],
                truncated: false,
            }),
        };
        let future = LocationView::from_snapshot(snapshot, 999);
        assert_eq!(future.status, FixStatus::Stale);
        assert_eq!(future.coordinates, None);
        assert_eq!(future.elevation_mm, None);
        assert_eq!(future.speed_mm_s, None);
        assert_eq!(future.course_millidegrees, None);
        assert_eq!(future.utc, None);
        assert_eq!(future.age_seconds, None);
        assert_eq!(future.receiver_status, ReceiverStatus::Quiet);
        assert_eq!(future.satellites, None);
        let valid = LocationView::from_snapshot(snapshot, 1_000);
        assert_eq!(valid.status, FixStatus::Valid);
        assert_eq!(valid.satellites_used, None);
        assert_eq!(valid.elevation_mm, Some(0));
        assert_eq!(valid.speed_mm_s, Some(1_000));
        assert_eq!(valid.course_millidegrees, Some(0));
        assert_eq!(valid.utc, UtcTime::new(0, 0, 0));
        assert_eq!(valid.receiver_status, ReceiverStatus::Active);
        assert_eq!(valid.satellites.unwrap().satellites_in_view, 0);
        snapshot.availability = LocationAvailability::Unavailable;
        let unavailable = LocationView::from_snapshot(snapshot, 1_000);
        assert_eq!(unavailable.speed_mm_s, None);
        assert_eq!(unavailable.course_millidegrees, None);
        assert_eq!(unavailable.utc, None);
        assert_eq!(
            LocationView::from_snapshot(snapshot, 1_000).elevation_mm,
            None
        );
        assert_eq!(
            LocationView::from_snapshot(snapshot, 1_000).coordinates,
            None
        );
        assert!(Coordinates::new(90_000_001, 0).is_none());
        assert!(Coordinates::new(0, -180_000_001).is_none());
        snapshot.availability = LocationAvailability::Ready;
        snapshot.observation.as_mut().unwrap().coordinates = None;
        assert_eq!(
            LocationView::from_snapshot(snapshot, 1_000).elevation_mm,
            None
        );
    }
}
