//! The automation file (`Automation.orb57autom`, `Automation.orb55autom`, …)
//! as the original PC Remote writes it: one block per event (5700i 3.0.1.20
//! and 5500i 3.1.0.2, captured 28 October 2026; 6300 4.1.1.19 and 8700i
//! 1.5.1.11, captured 29 October 2026; `5500PC.exe` 1.2.8.24 offers the
//! 5500i's functions in the same order and writes the same 5500.50 file).
use orban_protocol::adapter::DeviceModel;
use serde::{Deserialize, Serialize};

/// The client's non-preset actions, in its own order.
pub const FUNCTIONS: [&str; 18] = [
    "Bypass",
    "Exit Test",
    "Stereo",
    "Mono-R",
    "Mono-L",
    "Mono-Sum",
    "SSB",
    "Mod. Reduction 1",
    "Mod. Reduction 2",
    "Exit Mod. Reduction",
    "Analog Out Delay In",
    "Analog Out Delay Out",
    "Digital Out 1 Delay In",
    "Digital Out 1 Delay Out",
    "Digital Out 2 Delay In",
    "Digital Out 2 Delay Out",
    "Stereo Enc Delay In",
    "Stereo Enc Delay Out",
];
/// The functions the 5500i client 3.1.0.2 offers after the presets.
pub const FUNCTIONS_5500I: [&str; 18] = [
    "ST ENC DELAY: IN",
    "ST ENC DELAY: OUT",
    "AO SOURCE: FM",
    "AO SOURCE: FM+DEL",
    "AO SOURCE: MON",
    "DO SOURCE: FM",
    "DO SOURCE: FM+DEL",
    "DO SOURCE: MON",
    "Bypass",
    "Exit Test",
    "Stereo",
    "Mono-R",
    "Mono-L",
    "Mono-Sum",
    "SSB",
    "Mod. Reduction 1",
    "Mod. Reduction 2",
    "Exit Mod. Reduction",
];

/// The functions the 6300 client 4.1.1.19 offers after the presets (read
/// from its Automation dialog, 29 October 2026).
pub const FUNCTIONS_6300: [&str; 4] = ["Stereo", "Dual-Mono", "Bypass", "Exit Test"];

/// The functions the 2300 client 2.0.3.85 offers after the presets (its
/// Automation dialog, 29 October 2026).
pub const FUNCTIONS_2300: [&str; 9] = [
    "Bypass",
    "Exit Test",
    "Stereo",
    "Mono-R",
    "Mono-L",
    "Mono-Sum",
    "Mod. Reduction 1",
    "Mod. Reduction 2",
    "Exit Mod. Reduction",
];

/// The 8400's actions after its presets: Bypass and Exit Bypass (8400
/// manual 3.0.5, *System Setup/Automation*: "recall any factory or user
/// preset, and can activate Bypass mode ... or Exit Bypass").
pub const FUNCTIONS_8400: [&str; 2] = ["Bypass", "Exit Bypass"];

/// Whether the model's client function list is known.
pub fn available(model: DeviceModel) -> bool {
    matches!(
        model,
        DeviceModel::Optimod5700i
            | DeviceModel::Optimod5500i
            | DeviceModel::Optimod5500
            | DeviceModel::Optimod8700Hd
            | DeviceModel::Optimod6300
            | DeviceModel::Optimod8700i
            | DeviceModel::Optimod2300
            | DeviceModel::Optimod8400
    )
}

/// The model's functions, its document version and its file extension.
pub fn flavour(model: DeviceModel) -> (&'static [&'static str], &'static str, &'static str) {
    match model {
        DeviceModel::Optimod5500i | DeviceModel::Optimod5500 => {
            (&FUNCTIONS_5500I, "5500.50", "orb55autom")
        }
        // `8700PC.exe` 1.0.2.161 fills its list with the 5700i's 18 functions
        // in the same order (0x4ad720).
        DeviceModel::Optimod8700Hd => (&FUNCTIONS, "8700.51", "orb86autom"),
        DeviceModel::Optimod6300 => (&FUNCTIONS_6300, "6300.50", "orb63autom"),
        // `8700iPC.exe` 1.5.1.11 offers the 5700i's 18 functions in the same
        // order (its Automation dialog, 29 October 2026).
        DeviceModel::Optimod8700i => (&FUNCTIONS, "8700.52", "orb87autom"),
        DeviceModel::Optimod2300 => (&FUNCTIONS_2300, "2300.20", "orb23autom"),
        DeviceModel::Optimod8400 => (&FUNCTIONS_8400, "00.08", "aut"),
        _ => (&FUNCTIONS, "5700.51", "orb57autom"),
    }
}

const NO_FUNCTION: &str = "no function";

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const DAYS: [(&str, &str); 7] = [
    ("SUNDAY", "S"),
    ("MONDAY", "M"),
    ("TUESDAY", "T"),
    ("WEDNESDAY", "W"),
    ("THURSDAY", "T"),
    ("FRIDAY", "F"),
    ("SATURDAY", "S"),
];

/// One event: at `hour:minute:second`, either on the ticked weekdays
/// (`daily`) or on the date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
    pub weekdays: [bool; 7],
    pub daily: bool,
    /// A preset name or one of `FUNCTIONS`.
    pub action: String,
}

fn field<'a>(block: &'a str, name: &str) -> Result<&'a str, String> {
    let start = format!("C:<{name}>");
    let line = block
        .lines()
        .find(|l| l.starts_with(&start))
        .ok_or_else(|| format!("Automation event without {name}"))?;
    let rest = &line[start.len()..];
    let value = rest.split_once(':').map(|(_, v)| v).unwrap_or_default();
    let value = value.rsplit_once(";D:").map(|(v, _)| v).unwrap_or(value);
    Ok(value.trim_start_matches('<').trim_end_matches('>'))
}

fn number<T: std::str::FromStr>(text: &str, name: &str) -> Result<T, String> {
    text.trim_end_matches(':')
        .parse()
        .map_err(|_| format!("Invalid {name} in an automation event"))
}

pub fn parse(text: &str) -> Result<Vec<Event>, String> {
    let text = text.replace("\r\n", "\n");
    if text.contains("C:<SET BY WEEK>") {
        return parse_8400(&text);
    }
    text.split("End Preset<end>")
        .filter(|block| block.contains("C:<"))
        .map(|block| {
            let month = field(block, "AUTO SET MONTH")?;
            let mut weekdays = [false; 7];
            for (i, (name, _)) in DAYS.iter().enumerate() {
                weekdays[i] = field(block, name)? != "_";
            }
            Ok(Event {
                hour: number(field(block, "AUTO SET HOUR")?, "hour")?,
                minute: number(field(block, "AUTO SET MINUTE")?, "minute")?,
                second: number(field(block, "AUTO SET SECOND")?, "second")?,
                day: number(field(block, "AUTO SET DAY")?, "day")?,
                month: MONTHS
                    .iter()
                    .position(|m| *m == month)
                    .ok_or("Invalid month in an automation event")? as u8
                    + 1,
                year: number(field(block, "AUTO SET YEAR")?, "year")?,
                weekdays,
                daily: field(block, "SET BY DAY/WEEK")? == "DAY",
                // The client's "no function" is an event without an action.
                action: Some(field(block, "AUTOMATION EVENT")?)
                    .filter(|a| !a.eq_ignore_ascii_case(NO_FUNCTION))
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect()
}

/// The 8400's events: one `AUTO_<n>.aut` document per event in
/// `c:\\8400\\user\\`, ending `End Preset`, with `SET BY WEEK` and `SET BY
/// DATE` as separate switches (the original client's Add, 29 October 2026).
/// The files arrive here one after the other, in file order.
fn parse_8400(text: &str) -> Result<Vec<Event>, String> {
    text.split("End Preset\n")
        .filter(|block| block.contains("C:<"))
        .map(|block| {
            let month = field(block, "AUTO SET MONTH")?;
            let switch = |name: &str| -> Result<bool, String> {
                let start = format!("C:<{name}>");
                let line = block
                    .lines()
                    .find(|l| l.starts_with(&start))
                    .ok_or_else(|| format!("Automation event without {name}"))?;
                Ok(line.trim_end().ends_with(";D:1;"))
            };
            let mut weekdays = [false; 7];
            for (i, (name, _)) in DAYS.iter().enumerate() {
                weekdays[i] = field(block, name)? != "_";
            }
            Ok(Event {
                hour: number(field(block, "AUTO SET HOUR")?, "hour")?,
                minute: number(field(block, "AUTO SET MINUTE")?, "minute")?,
                second: number(field(block, "AUTO SET SECOND")?, "second")?,
                day: number(field(block, "AUTO SET DAY")?, "day")?,
                month: MONTHS
                    .iter()
                    .position(|m| *m == month)
                    .ok_or("Invalid month in an automation event")? as u8
                    + 1,
                year: number(field(block, "AUTO SET YEAR")?, "year")?,
                weekdays,
                daily: switch("SET BY WEEK")? && !switch("SET BY DATE")?,
                action: Some(field(block, "AUTOMATION EVENT")?)
                    .filter(|a| !a.eq_ignore_ascii_case(NO_FUNCTION))
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect()
}

/// One 8400 event file, `AUTO_<index>.aut`, as the original client writes
/// it. `action_index` is the action's place in the client's Event Type list
/// (0 = No Function).
pub fn render_8400(e: &Event, index: usize, action_index: usize) -> Result<String, String> {
    validate(e)?;
    let mut out = format!(
        "OptimodVersion=<00.08>\nPreset Name=<AUTO_{index}> size=16\nFactory Preset Name=<>\nPreset Type= <AUTOMATION PRESET>\n"
    );
    out.push_str(&format!(
        "C:<AUTO SET HOUR>String:<{:02}:>;D:{};\n",
        e.hour, e.hour
    ));
    out.push_str(&format!(
        "C:<AUTO SET MINUTE>String:<{:02}:>;D:{};\n",
        e.minute, e.minute
    ));
    out.push_str(&format!(
        "C:<AUTO SET SECOND>String:<{:02}>;D:{};\n",
        e.second, e.second
    ));
    out.push_str(&format!(
        "C:<AUTO SET DAY>String:<{:02}>;D:{};\n",
        e.day,
        e.day - 1
    ));
    out.push_str(&format!(
        "C:<AUTO SET MONTH>String:<{}>;D:{};\n",
        MONTHS[usize::from(e.month - 1)],
        e.month - 1
    ));
    out.push_str(&format!(
        "C:<AUTO SET YEAR>Int:{};D:{};\n",
        e.year,
        e.year - 2000
    ));
    for ((name, letter), on) in DAYS.iter().zip(e.weekdays) {
        out.push_str(&format!(
            "C:<{name}>String:<{}>;D:{};\n",
            if on { letter } else { "_" },
            u8::from(on)
        ));
    }
    out.push_str(&format!(
        "C:<SET BY WEEK>UserString:<>;D:{};\nC:<SET BY DATE>UserString:<>;D:{};\n",
        u8::from(e.daily),
        u8::from(!e.daily)
    ));
    out.push_str(&format!(
        "C:<AUTOMATION EVENT>UserString:<{}>;D:{action_index};\nEnd Preset\n",
        if e.action.is_empty() {
            "No Function"
        } else {
            &e.action
        }
    ));
    Ok(out)
}

fn validate(e: &Event) -> Result<(), String> {
    if e.hour > 23 || e.minute > 59 || e.second > 59 || !(1..=12).contains(&e.month) {
        return Err("Invalid time or date in an automation event".into());
    }
    if !(1..=31).contains(&e.day) || !(2000..=2099).contains(&e.year) {
        return Err("Invalid date in an automation event".into());
    }
    if e.action.len() > 40 || e.action.contains(['<', '>', ';', '\n', '\r']) {
        return Err("Invalid action in an automation event".into());
    }
    Ok(())
}

pub fn render(events: &[Event], model: DeviceModel) -> Result<String, String> {
    let (_, version, extension) = flavour(model);
    if events.len() > 100 {
        return Err("At most 100 automation events".into());
    }
    // The 8400 keeps one file per event; the service writes each one with
    // its action's place in the client's list.
    if model == DeviceModel::Optimod8400 {
        return events
            .iter()
            .enumerate()
            .map(|(index, e)| render_8400(e, index, 0))
            .collect();
    }
    let mut out = String::new();
    for (index, e) in events.iter().enumerate() {
        validate(e)?;
        out.push_str(&format!(
            "OptimodVersion=<{version}>\nPreset Name=<auto_event_{index:02}> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_{index:02}.{extension}>\n"
        ));
        out.push_str(&format!(
            "C:<AUTO SET MINUTE>String:<{:02}:>;D:{};\n",
            e.minute, e.minute
        ));
        out.push_str(&format!(
            "C:<AUTO SET HOUR>String:<{:02}:>;D:{};\n",
            e.hour, e.hour
        ));
        out.push_str(&format!(
            "C:<AUTO SET SECOND>String:<{:02}>;D:{};\n",
            e.second, e.second
        ));
        out.push_str(&format!(
            "C:<AUTO SET DAY>String:<{:02}>;D:{};\n",
            e.day,
            e.day - 1
        ));
        out.push_str(&format!(
            "C:<AUTO SET MONTH>String:<{}>;D:{};\n",
            MONTHS[usize::from(e.month - 1)],
            e.month - 1
        ));
        out.push_str(&format!("C:<AUTO SET YEAR>Int:{};D:{};\n", e.year, e.year));
        for ((name, letter), on) in DAYS.iter().zip(e.weekdays) {
            out.push_str(&format!(
                "C:<{name}>String:<{}>;D:{};\n",
                if on { letter } else { "_" },
                u8::from(on)
            ));
        }
        let (by, index) = if e.daily { ("DAY", 0) } else { ("DATE", 1) };
        out.push_str(&format!(
            "C:<SET BY DAY/WEEK>UserString:<{by}>;D:{index};\n"
        ));
        out.push_str(&format!(
            "C:<AUTOMATION EVENT>UserString:<{}>;D:0;\n",
            if e.action.is_empty() {
                NO_FUNCTION
            } else {
                &e.action
            }
        ));
        out.push_str("End Preset<end>\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// The file the original client uploaded for two events, the first daily
    /// without Sunday, the second on a date (28 October 2026).
    const CAPTURED: &str = "OptimodVersion=<5700.51>\nPreset Name=<auto_event_00> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_00.orb57autom>\nC:<AUTO SET MINUTE>String:<00:>;D:0;\nC:<AUTO SET HOUR>String:<00:>;D:0;\nC:<AUTO SET SECOND>String:<00>;D:0;\nC:<AUTO SET DAY>String:<28>;D:27;\nC:<AUTO SET MONTH>String:<OCT>;D:9;\nC:<AUTO SET YEAR>Int:2026;D:2026;\nC:<SUNDAY>String:<_>;D:0;\nC:<MONDAY>String:<M>;D:1;\nC:<TUESDAY>String:<T>;D:1;\nC:<WEDNESDAY>String:<W>;D:1;\nC:<THURSDAY>String:<T>;D:1;\nC:<FRIDAY>String:<F>;D:1;\nC:<SATURDAY>String:<S>;D:1;\nC:<SET BY DAY/WEEK>UserString:<DAY>;D:0;\nC:<AUTOMATION EVENT>UserString:<Exit Test>;D:0;\nEnd Preset<end>\nOptimodVersion=<5700.51>\nPreset Name=<auto_event_01> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_01.orb57autom>\nC:<AUTO SET MINUTE>String:<00:>;D:0;\nC:<AUTO SET HOUR>String:<00:>;D:0;\nC:<AUTO SET SECOND>String:<00>;D:0;\nC:<AUTO SET DAY>String:<28>;D:27;\nC:<AUTO SET MONTH>String:<OCT>;D:9;\nC:<AUTO SET YEAR>Int:2026;D:2026;\nC:<SUNDAY>String:<S>;D:1;\nC:<MONDAY>String:<M>;D:1;\nC:<TUESDAY>String:<T>;D:1;\nC:<WEDNESDAY>String:<W>;D:1;\nC:<THURSDAY>String:<T>;D:1;\nC:<FRIDAY>String:<F>;D:1;\nC:<SATURDAY>String:<S>;D:1;\nC:<SET BY DAY/WEEK>UserString:<DATE>;D:1;\nC:<AUTOMATION EVENT>UserString:<Exit Test>;D:0;\nEnd Preset<end>\n";

    #[test]
    fn the_captured_file_parses_and_renders_byte_for_byte() {
        let events = super::parse(CAPTURED).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].daily && !events[0].weekdays[0] && events[0].weekdays[6]);
        assert!(!events[1].daily);
        assert_eq!(
            (events[1].day, events[1].month, events[1].year),
            (28, 10, 2026)
        );
        assert_eq!(
            super::render(&events, super::DeviceModel::Optimod5700i).unwrap(),
            CAPTURED
        );
        assert_eq!(
            super::render(&[], super::DeviceModel::Optimod5700i).unwrap(),
            ""
        );
    }

    /// The 5500i client's file for its default new event (Backup to PC,
    /// 28 October 2026).
    #[test]
    fn renders_the_5500i_clients_new_event() {
        const CAPTURED_5500I: &str = "OptimodVersion=<5500.50>\nPreset Name=<auto_event_00> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_00.orb55autom>\nC:<AUTO SET MINUTE>String:<00:>;D:0;\nC:<AUTO SET HOUR>String:<00:>;D:0;\nC:<AUTO SET SECOND>String:<00>;D:0;\nC:<AUTO SET DAY>String:<01>;D:0;\nC:<AUTO SET MONTH>String:<JAN>;D:0;\nC:<AUTO SET YEAR>Int:2005;D:2005;\nC:<SUNDAY>String:<S>;D:1;\nC:<MONDAY>String:<M>;D:1;\nC:<TUESDAY>String:<T>;D:1;\nC:<WEDNESDAY>String:<W>;D:1;\nC:<THURSDAY>String:<T>;D:1;\nC:<FRIDAY>String:<F>;D:1;\nC:<SATURDAY>String:<S>;D:1;\nC:<SET BY DAY/WEEK>UserString:<DAY>;D:0;\nC:<AUTOMATION EVENT>UserString:<no function>;D:0;\nEnd Preset<end>\n";
        let events = super::parse(CAPTURED_5500I).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            super::render(&events, super::DeviceModel::Optimod5500i).unwrap(),
            CAPTURED_5500I
        );
    }

    /// The new event the 6300 and 8700i clients saved (29 October 2026):
    /// the same blocks with their own version and extension.
    #[test]
    fn renders_the_6300_and_8700i_clients_new_event() {
        for (model, version, extension) in [
            (super::DeviceModel::Optimod6300, "6300.50", "orb63autom"),
            (super::DeviceModel::Optimod8700i, "8700.52", "orb87autom"),
        ] {
            let captured = format!(
                "OptimodVersion=<{version}>\nPreset Name=<auto_event_00> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_00.{extension}>\nC:<AUTO SET MINUTE>String:<00:>;D:0;\nC:<AUTO SET HOUR>String:<00:>;D:0;\nC:<AUTO SET SECOND>String:<00>;D:0;\nC:<AUTO SET DAY>String:<29>;D:28;\nC:<AUTO SET MONTH>String:<OCT>;D:9;\nC:<AUTO SET YEAR>Int:2026;D:2026;\nC:<SUNDAY>String:<S>;D:1;\nC:<MONDAY>String:<M>;D:1;\nC:<TUESDAY>String:<T>;D:1;\nC:<WEDNESDAY>String:<W>;D:1;\nC:<THURSDAY>String:<T>;D:1;\nC:<FRIDAY>String:<F>;D:1;\nC:<SATURDAY>String:<S>;D:1;\nC:<SET BY DAY/WEEK>UserString:<DAY>;D:0;\nC:<AUTOMATION EVENT>UserString:<no function>;D:0;\nEnd Preset<end>\n"
            );
            let events = super::parse(&captured).unwrap();
            assert_eq!(super::render(&events, model).unwrap(), captured);
            assert!(super::available(model));
        }
        assert_eq!(
            super::flavour(super::DeviceModel::Optimod6300).0,
            super::FUNCTIONS_6300
        );
        assert_eq!(
            super::flavour(super::DeviceModel::Optimod8700i).0,
            super::FUNCTIONS
        );
    }

    /// The file the original 8400 client wrote for its new event (Add, a
    /// factory preset, every day, 29 October 2026), line ends as sent.
    #[test]
    fn renders_the_8400_clients_new_event() {
        const CAPTURED_8400: &str = "OptimodVersion=<00.08>\nPreset Name=<AUTO_0> size=16\nFactory Preset Name=<>\nPreset Type= <AUTOMATION PRESET>\nC:<AUTO SET HOUR>String:<00:>;D:0;\nC:<AUTO SET MINUTE>String:<00:>;D:0;\nC:<AUTO SET SECOND>String:<00>;D:0;\nC:<AUTO SET DAY>String:<01>;D:0;\nC:<AUTO SET MONTH>String:<FEB>;D:1;\nC:<AUTO SET YEAR>Int:2000;D:0;\nC:<SUNDAY>String:<S>;D:1;\nC:<MONDAY>String:<M>;D:1;\nC:<TUESDAY>String:<T>;D:1;\nC:<WEDNESDAY>String:<W>;D:1;\nC:<THURSDAY>String:<T>;D:1;\nC:<FRIDAY>String:<F>;D:1;\nC:<SATURDAY>String:<S>;D:1;\nC:<SET BY WEEK>UserString:<>;D:1;\nC:<SET BY DATE>UserString:<>;D:0;\nC:<AUTOMATION EVENT>UserString:<CLASSICAL-2 BAND>;D:1;\nEnd Preset\n";
        let events = super::parse(CAPTURED_8400).unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].daily && events[0].weekdays.iter().all(|d| *d));
        assert_eq!((events[0].month, events[0].year), (2, 2000));
        assert_eq!(events[0].action, "CLASSICAL-2 BAND");
        assert_eq!(super::render_8400(&events[0], 0, 1).unwrap(), CAPTURED_8400);
        assert!(super::available(super::DeviceModel::Optimod8400));
    }
}
