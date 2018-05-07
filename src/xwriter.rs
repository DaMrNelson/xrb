use models::*;

pub trait XBufferedWriter {
    fn write_flush(&mut self);
    fn write_pad(&mut self, len: usize);
    fn write_bool(&mut self, input: bool);
    fn write_u8(&mut self, input: u8);
    fn write_i16(&mut self, input: i16);
    fn write_u16(&mut self, input: u16);
    fn write_i32(&mut self, input: i32);
    fn write_u32(&mut self, input: u32);
    fn write_val_bool(&mut self, input: bool);
    fn write_val_u8(&mut self, input: u8);
    fn write_val_i16(&mut self, input: i16);
    fn write_val_u16(&mut self, input: u16);
    fn write_val_i32(&mut self, input: i32);
    fn write_val_u32(&mut self, input: u32);
    fn write_val(&mut self, input: u32);
    fn write_values<T: Value>(&mut self, values: &Vec<T>);
    fn read_pad(&mut self, len: usize);
    fn read_bool(&mut self) -> bool;
    fn read_u8(&mut self) -> u8;
    fn read_i16(&mut self) -> i16;
    fn read_u16(&mut self) -> u16;
    fn read_u32(&mut self) -> u32;
    fn read_char(&mut self) -> char;
    fn read_str(&mut self, len: usize) -> String;
}