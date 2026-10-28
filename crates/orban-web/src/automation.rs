//! The 5700i automation file (`Automation.orb57autom`) as the original PC
//! Remote 3.0.1.20 writes it: one block per event (captured 28 October 2026).
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
                action: field(block, "AUTOMATION EVENT")?.to_owned(),
            })
        })
        .collect()
}

pub fn render(events: &[Event]) -> Result<String, String> {
    if events.len() > 100 {
        return Err("At most 100 automation events".into());
    }
    let mut out = String::new();
    for (index, e) in events.iter().enumerate() {
        if e.hour > 23 || e.minute > 59 || e.second > 59 || !(1..=12).contains(&e.month) {
            return Err("Invalid time or date in an automation event".into());
        }
        if !(1..=31).contains(&e.day) || !(2000..=2099).contains(&e.year) {
            return Err("Invalid date in an automation event".into());
        }
        if e.action.is_empty()
            || e.action.len() > 40
            || e.action.contains(['<', '>', ';', '\n', '\r'])
        {
            return Err("Invalid action in an automation event".into());
        }
        out.push_str(&format!(
            "OptimodVersion=<5700.51>\nPreset Name=<auto_event_{index:02}> size=17\nFactory Preset Name=<Automation Preset>\nPreset Type= <AUTOMATION PRESET>\nPreset File Name= <auto_event_{index:02}.orb57autom>\n"
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
            e.action
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
        assert_eq!(super::render(&events).unwrap(), CAPTURED);
        assert_eq!(super::render(&[]).unwrap(), "");
    }
}
