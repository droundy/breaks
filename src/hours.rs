use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::time::Duration;

/// Deserializes a `Duration` or `SystemTime` via the humantime crate.
///
/// This function can be used with `serde_derive`'s `with` and
/// `deserialize_with` annotations.
pub fn deserialize<'a, T, D>(d: D) -> Result<T, D::Error>
where
    Serde<T>: Deserialize<'a>,
    D: Deserializer<'a>,
{
    Serde::deserialize(d).map(Serde::into_inner)
}

/// Serializes a `Duration` or `SystemTime` via the humantime crate.
///
/// This function can be used with `serde_derive`'s `with` and
/// `serialize_with` annotations.
pub fn serialize<T, S>(d: &T, s: S) -> Result<S::Ok, S::Error>
where
    for<'a> Serde<&'a T>: Serialize,
    S: Serializer,
{
    Serde::from(d).serialize(s)
}

/// A wrapper type which implements `Serialize` and `Deserialize` for
/// types involving `SystemTime` and `Duration`.
#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub struct Serde<T>(T);

impl<T> fmt::Debug for Serde<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(formatter)
    }
}

impl<T> std::ops::Deref for Serde<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Serde<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Serde<T> {
    /// Consumes the `De`, returning the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Serde<T> {
    fn from(val: T) -> Serde<T> {
        Serde(val)
    }
}

impl<'de> Deserialize<'de> for Serde<Duration> {
    fn deserialize<D>(d: D) -> Result<Serde<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de2> de::Visitor<'de2> for V {
            type Value = Duration;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("a duration")
            }

            fn visit_str<E>(self, v: &str) -> Result<Duration, E>
            where
                E: de::Error,
            {
                parseme(v).map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
            }
        }

        d.deserialize_str(V).map(Serde)
    }
}

impl<'de> Deserialize<'de> for Serde<Option<Duration>> {
    fn deserialize<D>(d: D) -> Result<Serde<Option<Duration>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<Serde<Duration>>::deserialize(d)? {
            Some(Serde(dur)) => Ok(Serde(Some(dur))),
            None => Ok(Serde(None)),
        }
    }
}

impl<'a> ser::Serialize for Serde<&'a Duration> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        tostring(*self.0).serialize(serializer)
    }
}

fn parseme(v: &str) -> Result<Duration, ()> {
    let mut hours = 0.0;
    let mut minutes = 0.0;
    if let Some((h, m)) = v.split_once(":") {
        hours = h.trim().parse().map_err(|_| ())?;
        minutes = m.trim().parse().map_err(|_| ())?;
    } else if let Some(h) = v
        .strip_suffix("h")
        .or_else(|| v.strip_suffix("hours"))
        .or_else(|| v.strip_suffix("hour"))
    {
        hours = h.trim().parse().map_err(|_| ())?;
    } else if let Some(m) = v
        .strip_suffix("m")
        .or_else(|| v.strip_suffix("minutes"))
        .or_else(|| v.strip_suffix("minute"))
    {
        minutes = m.trim().parse().map_err(|_| ())?;
    } else {
        return Err(());
    }
    Ok(Duration::from_secs_f64((hours * 60.0 + minutes) * 60.0))
}

fn tostring(x: Duration) -> String {
    let secs = x.as_secs();
    let minutes = secs / 60;
    let hours = minutes / 60;
    let minutes = minutes - hours * 60;
    match (hours, minutes) {
        (0, 0) => "0 minutes".to_string(),
        (1, 0) => format!("{hours} hour"),
        (_, 0) => format!("{hours} hours"),
        (0, 1) => format!("{minutes} minute"),
        (0, _) => format!("{minutes} minutes"),
        _ => format!("{hours}:{minutes:02}"),
    }
}

impl ser::Serialize for Serde<Duration> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        tostring(self.0).serialize(serializer)
    }
}

impl ser::Serialize for Serde<Option<Duration>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        Serde(&self.0).serialize(serializer)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pm() {
        assert_eq!(parseme("1:00").unwrap(), Duration::from_secs(60 * 60));
        assert_eq!(parseme("1 hour").unwrap(), Duration::from_secs(60 * 60));
        assert_eq!(parseme("2 minutes").unwrap(), Duration::from_secs(2 * 60));
    }

    #[test]
    fn ts() {
        assert_eq!(tostring(Duration::from_secs(60)).as_str(), "1 minute");
        assert_eq!(tostring(Duration::from_secs(2 * 60)).as_str(), "2 minutes");
        assert_eq!(
            tostring(Duration::from_secs(3 * 60 * 60)).as_str(),
            "3 hours"
        );
        assert_eq!(
            tostring(Duration::from_secs(3 * 60 * 60 + 2 * 60)).as_str(),
            "3:02"
        );
    }

    #[test]
    fn with() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "super")]
            time: Duration,
        }

        let json = r#"{"time": "2 hours"}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, Duration::from_secs(120 * 60));
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":"2 hours"}"#);

        let json = r#"{"time": "2:00"}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, Duration::from_secs(120 * 60));
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":"2 hours"}"#);
    }
}
