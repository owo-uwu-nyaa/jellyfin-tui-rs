use std::mem;

use ratatui_core::{
    style::Style,
    text::{Line, Span, Text},
};
use serde::{
    Serialize, Serializer,
    de::{Error as _, Unexpected},
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
};
use serde_json::Error;

struct RatatuiSerializer {
    text: Text<'static>,
    cur_line: Line<'static>,
    first_container_line: bool,
    style: JsonStyle,
    level: usize,
}

pub struct JsonStyle {
    keyword: Style,
    number: Style,
    string: Style,
    symbols: Style,
    map_key: Style,
}

impl JsonStyle {
    pub fn new(
        keyword: impl Into<Style>,
        number: impl Into<Style>,
        string: impl Into<Style>,
        symbols: impl Into<Style>,
        map_key: impl Into<Style>,
    ) -> Self {
        Self {
            keyword: keyword.into(),
            number: number.into(),
            string: string.into(),
            symbols: symbols.into(),
            map_key: map_key.into(),
        }
    }
}

pub fn serialize_to_tui(style: JsonStyle, val: impl Serialize) -> Result<Text<'static>, Error> {
    let mut serializer = RatatuiSerializer {
        text: Text::default(),
        cur_line: Line::default(),
        first_container_line: false,
        style,
        level: 0,
    };
    val.serialize(&mut serializer)?;
    serializer.text.push_line(serializer.cur_line);
    Ok(serializer.text)
}

impl Serializer for &mut RatatuiSerializer {
    type Ok = ();

    type Error = Error;

    type SerializeSeq = Self;

    type SerializeTuple = Self;

    type SerializeTupleStruct = Self;

    type SerializeTupleVariant = Self;

    type SerializeMap = Self;

    type SerializeStruct = Self;

    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        let v: &'static str = if v { "true" } else { "false" };
        self.cur_line.push_span(Span::styled(v, self.style.keyword));
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v.into())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.cur_line
            .push_span(Span::styled(v.to_string(), self.style.number));
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v.into())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.cur_line
            .push_span(Span::styled(v.to_string(), self.style.number));
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v.into())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.cur_line
            .push_span(Span::styled(v.to_string(), self.style.number));
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.cur_line
            .push_span(Span::styled(serde_json::to_string(v)?, self.style.string));
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let span = if v.len() > 48 {
            Span::styled(r#""<binary>""#, self.style.string)
        } else {
            let mut s = String::with_capacity(2 + v.len() * 2);
            s.push('"');
            for v in v {
                fn c(v: u8) -> char {
                    (b"0123456789abcdef"[v as usize]).into()
                }
                s.push(c(v & 15));
                s.push(c((v >> 4) & 15));
            }
            s.push('"');
            Span::styled(s, self.style.string)
        };
        self.cur_line.push_span(span);
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.cur_line
            .push_span(Span::styled("null", self.style.keyword));
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.serialize_map(None)?;
        SerializeMap::serialize_key(&mut self, variant)?;
        SerializeMap::serialize_value(&mut self, value)?;
        SerializeMap::end(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.cur_line
            .push_span(Span::styled("[", self.style.symbols));
        self.first_container_line = true;
        self.level += 1;
        let last_line = mem::replace(
            &mut self.cur_line,
            Span::raw("  ".repeat(self.level)).into(),
        );
        self.text.push_line(last_line);
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(None)
    }

    #[inline(always)]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.serialize_map(None)?;
        SerializeMap::serialize_key(&mut self, variant)?;
        self.serialize_tuple(len)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.cur_line
            .push_span(Span::styled("{", self.style.symbols));
        self.first_container_line = true;
        self.level += 1;
        let last_line = mem::replace(
            &mut self.cur_line,
            Span::raw("  ".repeat(self.level)).into(),
        );
        self.text.push_line(last_line);
        Ok(self)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(None)
    }

    fn serialize_struct_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.serialize_map(None)?;
        SerializeMap::serialize_key(&mut self, variant)?;
        self.serialize_map(None)
    }
}

impl SerializeSeq for &mut RatatuiSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        if !self.first_container_line {
            self.cur_line
                .push_span(Span::styled(",", self.style.symbols));
        }
        self.first_container_line = false;
        let last_line = mem::replace(
            &mut self.cur_line,
            Span::raw("  ".repeat(self.level)).into(),
        );
        self.text.push_line(last_line);
        let s: &mut RatatuiSerializer = self;
        value.serialize(s)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.level = self.level.saturating_sub(1);
        self.first_container_line = false;
        let last_line = mem::replace(
            &mut self.cur_line,
            Span::raw("  ".repeat(self.level)).into(),
        );
        self.text.push_line(last_line);
        self.cur_line
            .push_span(Span::styled("]", self.style.symbols));
        Ok(())
    }
}

impl SerializeTuple for &mut RatatuiSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for &mut RatatuiSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleVariant for &mut RatatuiSerializer {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(&mut *self)?;
        SerializeMap::end(self)
    }
}

impl SerializeMap for &mut RatatuiSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        if !self.first_container_line {
            self.cur_line
                .push_span(Span::styled(",", self.style.symbols));
        }
        self.first_container_line = false;
        let last_line = mem::replace(
            &mut self.cur_line,
            Span::raw("  ".repeat(self.level)).into(),
        );
        self.text.push_line(last_line);
        self.cur_line.push_span(key.serialize(KeySerializer {
            map_key: self.style.map_key,
        })?);
        self.cur_line
            .push_span(Span::styled(":", self.style.symbols));
        self.cur_line.push_span(Span::raw(" "));
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        let s: &mut RatatuiSerializer = self;
        value.serialize(s)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.level = self.level.saturating_sub(1);
        self.first_container_line = false;
        let last_line = mem::replace(
            &mut self.cur_line,
            Span::raw("  ".repeat(self.level)).into(),
        );
        self.text.push_line(last_line);
        self.cur_line
            .push_span(Span::styled("}", self.style.symbols));
        Ok(())
    }
}

impl SerializeStruct for &mut RatatuiSerializer {
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        SerializeMap::serialize_key(self, key)?;
        SerializeMap::serialize_value(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeMap::end(self)
    }
}

impl SerializeStructVariant for &mut RatatuiSerializer {
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeMap::end(&mut *self)?;
        SerializeMap::end(self)
    }
}

struct KeySerializer {
    map_key: Style,
}
impl Serializer for KeySerializer {
    type Ok = Span<'static>;

    type Error = Error;

    type SerializeSeq = serde::ser::Impossible<Self::Ok, Self::Error>;

    type SerializeTuple = serde::ser::Impossible<Self::Ok, Self::Error>;

    type SerializeTupleStruct = serde::ser::Impossible<Self::Ok, Self::Error>;

    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;

    type SerializeStruct = serde::ser::Impossible<Self::Ok, Self::Error>;

    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Bool(v),
            &"string key",
        ))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Signed(v),
            &"string key",
        ))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Unsigned(v),
            &"string key",
        ))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Float(v),
            &"string key",
        ))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Char(v),
            &"string key",
        ))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Span::styled(v.to_string(), self.map_key))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Bytes(v),
            &"string key",
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_value(Unexpected::Option, &"string key"))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Unit,
            &"string key",
        ))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Unit,
            &"string key",
        ))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Span::styled(variant, self.map_key))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        Err(Error::invalid_type(
            serde::de::Unexpected::NewtypeVariant,
            &"string key",
        ))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Seq,
            &"string key",
        ))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Seq,
            &"string key",
        ))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Seq,
            &"string key",
        ))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::TupleVariant,
            &"string key",
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Map,
            &"string key",
        ))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::Map,
            &"string key",
        ))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::invalid_type(
            serde::de::Unexpected::StructVariant,
            &"string key",
        ))
    }
}
