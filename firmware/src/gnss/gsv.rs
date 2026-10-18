//! Complete GSV groups with bounded storage and independent expiry.

use paper_mesh_platform::location::{
    MAX_SATELLITES, SatelliteObservation, SatelliteSignal, SatelliteSystem,
};

use super::receiver::decimal;

pub const MAX_AGE_MS: u64 = 5_000;
const STREAMS: usize = 12;
const PER_STREAM: usize = 36;
const UNIQUE_SATELLITES: usize = 96;

#[derive(Clone, Copy)]
struct Group {
    satellites: [Option<SatelliteSignal>; PER_STREAM],
    count: usize,
    started_at_ms: u64,
    total: u8,
    next: u8,
}

impl Group {
    const EMPTY: Self = Self {
        satellites: [None; PER_STREAM],
        count: 0,
        started_at_ms: 0,
        total: 0,
        next: 1,
    };
}

#[derive(Clone, Copy)]
struct Stream {
    key: Option<(SatelliteSystem, u8)>,
    pending: Option<Group>,
    complete: Group,
    received_at_ms: Option<u64>,
}

impl Stream {
    const EMPTY: Self = Self {
        key: None,
        pending: None,
        complete: Group::EMPTY,
        received_at_ms: None,
    };
}

pub struct Gsv {
    streams: [Stream; STREAMS],
    overflow_at_ms: Option<u64>,
}

impl Gsv {
    pub const EMPTY: Self = Self {
        streams: [Stream::EMPTY; STREAMS],
        overflow_at_ms: None,
    };

    pub fn clear(&mut self) {
        for stream in &mut self.streams {
            *stream = Stream::EMPTY;
        }
        self.overflow_at_ms = None;
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        self.streams
            .iter()
            .filter_map(|stream| stream.received_at_ms)
            .chain(self.overflow_at_ms)
            .map(|time| time.saturating_add(MAX_AGE_MS))
            .min()
    }

    pub fn expire(&mut self, now_ms: u64) -> bool {
        let mut changed = false;
        for stream in &mut self.streams {
            if stream
                .received_at_ms
                .is_some_and(|time| now_ms.saturating_sub(time) >= MAX_AGE_MS)
            {
                stream.received_at_ms = None;
                changed = true;
            }
            if stream
                .pending
                .is_some_and(|group| now_ms.saturating_sub(group.started_at_ms) >= MAX_AGE_MS)
            {
                stream.pending = None;
            }
            if stream.received_at_ms.is_none() && stream.pending.is_none() {
                stream.key = None;
            }
        }
        if self
            .overflow_at_ms
            .is_some_and(|time| now_ms.saturating_sub(time) >= MAX_AGE_MS)
        {
            self.overflow_at_ms = None;
            changed = true;
        }
        changed
    }

    pub fn feed(&mut self, fields: &[&str], now_ms: u64) {
        self.expire(now_ms);
        let Some((key, total, number, count, entries)) = parse(fields) else {
            // A malformed group must not complete an earlier partial group.
            if let Some(system) = fields.first().and_then(|field| system(field)) {
                for stream in &mut self.streams {
                    if stream.key.is_some_and(|key| key.0 == system) {
                        stream.pending = None;
                    }
                }
            }
            return;
        };
        let index = self
            .streams
            .iter()
            .position(|stream| stream.key == Some(key));
        let index = index.or_else(|| self.streams.iter().position(|stream| stream.key.is_none()));
        let Some(index) = index else {
            self.overflow_at_ms = Some(now_ms);
            return;
        };
        let stream = &mut self.streams[index];
        stream.key = Some(key);
        if number == 1 {
            stream.pending = Some(Group {
                total,
                count,
                started_at_ms: now_ms,
                ..Group::EMPTY
            });
        }
        let Some(group) = stream.pending.as_mut() else {
            return;
        };
        if group.total != total || group.count != count || group.next != number {
            stream.pending = None;
            return;
        }
        let offset = usize::from(number - 1) * 4;
        for (index, entry) in entries.into_iter().enumerate() {
            if let Some(entry) = entry {
                if group.satellites[..offset + index]
                    .iter()
                    .flatten()
                    .any(|previous| previous.id == entry.id)
                {
                    stream.pending = None;
                    return;
                }
                group.satellites[offset + index] = Some(entry);
            }
        }
        group.next += 1;
        if number == total {
            stream.complete = *group;
            stream.received_at_ms = Some(now_ms);
            stream.pending = None;
        }
    }

    pub fn observation(&self) -> Option<SatelliteObservation> {
        let mut unique: [Option<SatelliteSignal>; UNIQUE_SATELLITES] = [None; UNIQUE_SATELLITES];
        let mut count = 0;
        let mut oldest = None;
        let mut truncated = self.overflow_at_ms.is_some();
        for stream in &self.streams {
            let Some(time) = stream.received_at_ms else {
                continue;
            };
            oldest = Some(oldest.map_or(time, |previous: u64| previous.min(time)));
            for satellite in stream.complete.satellites.iter().flatten() {
                if let Some(previous) = unique[..count].iter_mut().flatten().find(|previous| {
                    previous.system == satellite.system && previous.id == satellite.id
                }) {
                    previous.cn0_db_hz = previous.cn0_db_hz.max(satellite.cn0_db_hz);
                } else if count < UNIQUE_SATELLITES {
                    unique[count] = Some(*satellite);
                    count += 1;
                } else {
                    truncated = true;
                }
            }
        }
        let observed_at_ms = oldest?;
        unique[..count].sort_unstable_by_key(|satellite| {
            let satellite = satellite.unwrap();
            (
                core::cmp::Reverse(satellite.cn0_db_hz),
                satellite.system as u8,
                satellite.id,
            )
        });
        let mut strongest = [None; MAX_SATELLITES];
        let displayed = count.min(MAX_SATELLITES);
        strongest[..displayed].copy_from_slice(&unique[..displayed]);
        Some(SatelliteObservation {
            observed_at_ms,
            satellites_in_view: count as u16,
            satellites_with_signal: unique[..count]
                .iter()
                .flatten()
                .filter(|satellite| satellite.cn0_db_hz.is_some_and(|level| level > 0))
                .count() as u16,
            strongest,
            truncated,
        })
    }
}

fn system(header: &str) -> Option<SatelliteSystem> {
    Some(match header {
        "GPGSV" => SatelliteSystem::Gps,
        "GLGSV" => SatelliteSystem::Glonass,
        "GAGSV" => SatelliteSystem::Galileo,
        "GBGSV" | "BDGSV" => SatelliteSystem::BeiDou,
        "GQGSV" => SatelliteSystem::Qzss,
        "GIGSV" => SatelliteSystem::Navic,
        _ => return None,
    })
}

type Parsed = (
    (SatelliteSystem, u8),
    u8,
    u8,
    usize,
    [Option<SatelliteSignal>; 4],
);

fn parse(fields: &[&str]) -> Option<Parsed> {
    let system = system(fields.first()?)?;
    let total = decimal(fields.get(1)?)?;
    let number = decimal(fields.get(2)?)?;
    let count = decimal(fields.get(3)?)? as usize;
    if !(1..=9).contains(&total)
        || number == 0
        || number > total
        || count > PER_STREAM
        || total as usize != count.div_ceil(4).max(1)
    {
        return None;
    }
    let tail = &fields[4..];
    let (blocks, signal) = match tail.len() % 4 {
        0 => (tail, 0),
        1 => {
            let value = tail.last()?;
            let signal = if value.is_empty() {
                0
            } else {
                if value.len() != 1 {
                    return None;
                }
                u8::from_str_radix(value, 16).ok()?
            };
            if signal > 15 {
                return None;
            }
            (&tail[..tail.len() - 1], signal)
        }
        _ => return None,
    };
    let expected = count.saturating_sub((number as usize - 1) * 4).min(4);
    if blocks.len() < expected * 4 || blocks.len() > 16 {
        return None;
    }
    let mut entries = [None; 4];
    for (index, block) in blocks.chunks_exact(4).enumerate() {
        if index >= expected {
            if block.iter().any(|field| !field.is_empty()) {
                return None;
            }
            continue;
        }
        let id = decimal(block[0])?;
        if id == 0 || id > 999 {
            return None;
        }
        if (!block[1].is_empty() && decimal(block[1])? > 90)
            || (!block[2].is_empty() && decimal(block[2])? > 359)
        {
            return None;
        }
        let cn0_db_hz = if block[3].is_empty() {
            None
        } else {
            let level = decimal(block[3])?;
            if level > 99 {
                return None;
            }
            Some(level as u8)
        };
        entries[index] = Some(SatelliteSignal {
            system,
            id: id as u16,
            cn0_db_hz,
        });
    }
    Some(((system, signal), total as u8, number as u8, count, entries))
}
