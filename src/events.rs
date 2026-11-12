use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Restart,
    Crash,
    Update,
    Service,
    Other,
}

impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Restart,
        Self::Crash,
        Self::Update,
        Self::Service,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Restart => "Başlatma / kapanma",
            Self::Crash => "Uygulama çökmesi",
            Self::Update => "Windows Update",
            Self::Service => "Servis",
            Self::Other => "Diğer",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub time: DateTime<Utc>,
    pub computer: String,
    pub channel: String,
    pub provider: String,
    pub id: u32,
    pub record: Option<u64>,
    pub level: Option<u8>,
    pub source: String,
    pub message: String,
    pub xml: String,
    pub clipped: bool,
}

impl Event {
    fn from(&self, provider: &str) -> bool {
        self.provider.eq_ignore_ascii_case(provider)
    }

    pub fn kind(&self) -> Kind {
        if self.boot_marker()
            || (self.from("Microsoft-Windows-Kernel-General") && self.id == 13)
            || (self.from("EventLog") && self.id == 6006)
            || (self.from("User32") && self.id == 1074)
        {
            Kind::Restart
        } else if (self.from("Application Error") && self.id == 1000)
            || (self.from("Application Hang") && self.id == 1002)
            || (self.from("Windows Error Reporting") && self.id == 1001)
            || (self.from(".NET Runtime") && self.id == 1026)
        {
            Kind::Crash
        } else if self.from("Microsoft-Windows-WindowsUpdateClient") {
            Kind::Update
        } else if self.from("Service Control Manager") {
            Kind::Service
        } else {
            Kind::Other
        }
    }

    pub fn boot_marker(&self) -> bool {
        (self.from("Microsoft-Windows-Kernel-General") && self.id == 12)
            || (self.from("Microsoft-Windows-Kernel-Power") && self.id == 41)
            || (self.from("EventLog") && matches!(self.id, 6005 | 6008))
    }

    pub fn level_label(&self) -> &'static str {
        match self.level {
            Some(1) => "Kritik",
            Some(2) => "Hata",
            Some(3) => "Uyarı",
            Some(4) => "Bilgi",
            Some(5) => "Ayrıntılı",
            _ => "—",
        }
    }
}

#[derive(Default, Deserialize)]
pub struct TimelineData {
    pub events: Vec<Event>,
    pub warnings: Vec<String>,
    pub opened: usize,
}

impl TimelineData {
    pub fn organize(&mut self) {
        self.events.sort_by_key(|event| event.time);
        let mut known = HashSet::new();
        self.events.retain(|event| {
            event.record.is_none_or(|record| {
                known.insert((
                    event.computer.to_ascii_lowercase(),
                    event.channel.to_ascii_lowercase(),
                    event.provider.to_ascii_lowercase(),
                    event.id,
                    record,
                    event.time,
                ))
            })
        });
    }

    pub fn last_boot(&self) -> Option<usize> {
        self.events.iter().rposition(Event::boot_marker)
    }

    pub fn near(&self, event: &Event, anchor: usize) -> bool {
        let anchor = &self.events[anchor];
        event.computer.eq_ignore_ascii_case(&anchor.computer)
            && (event.time - anchor.time).abs() <= Duration::minutes(30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(provider: &str, id: u32, minute: u32, computer: &str) -> Event {
        Event {
            time: chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 11, 11, 12, minute, 0).unwrap(),
            computer: computer.into(),
            channel: "System".into(),
            provider: provider.into(),
            id,
            record: Some(u64::from(id)),
            level: Some(2),
            source: "System".into(),
            message: String::new(),
            xml: String::new(),
            clipped: false,
        }
    }

    #[test]
    fn orders_events_and_keeps_restart_context_on_one_computer() {
        let mut data = TimelineData {
            events: vec![
                event("Application Error", 1000, 16, "B"),
                event("User32", 1074, 17, "A"),
                event("Application Error", 1000, 13, "A"),
                event("Microsoft-Windows-Kernel-General", 12, 15, "A"),
                event("Application Error", 1000, 13, "A"),
            ],
            ..Default::default()
        };
        data.organize();
        assert_eq!(data.events.len(), 4);
        assert_eq!(data.events[0].kind(), Kind::Crash);
        assert_eq!(data.events[3].kind(), Kind::Restart);
        let anchor = data.last_boot().unwrap();
        assert_eq!(data.events[anchor].id, 12);
        assert!(data.near(&data.events[0], anchor));
        assert!(!data.near(&data.events[2], anchor));
    }
}
