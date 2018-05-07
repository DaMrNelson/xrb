use std::mem::discriminant;

use XClient;
use xwriter::XBufferedWriter;

// Root trait for all values (ie GraphicsContextValue)
pub trait Value {
    fn get_mask(&self) -> u32;
    fn write<T: XBufferedWriter>(&self, client: &mut T);
}

// Root trait for all valued types
pub trait Valued {
    fn val(&self) -> u32;
}

////////////////////////////////////////
/// XRB TYPES
////////////////////////////////////////


#[derive(Debug)]
pub struct ConnectInfo {
    pub status_code: u8,
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub additional_data_len: u16,
    pub release_number: u32,
    pub resource_id_base: u32,
    pub resource_id_mask: u32,
    pub motion_buffer_size: u32,
    pub max_request_length: u16,
    pub num_screens: u8,
    pub num_formats: u8,
    pub image_byte_order: ByteOrder,
    pub bitmap_format_bit_order: BitOrder,
    pub bitmap_format_scanline_unit: u8,
    pub bitmap_format_scanline_pad: u8,
    pub min_keycode: char,
    pub max_keycode: char,
    pub vendor: String,
    pub formats: Vec<Format>,
    pub screens: Vec<Screen>
}

impl ConnectInfo {
    pub fn empty() -> ConnectInfo {
        ConnectInfo {
            status_code: 0,
            protocol_major_version: 0,
            protocol_minor_version: 0,
            additional_data_len: 0,
            release_number: 0,
            resource_id_base: 0,
            resource_id_mask: 0,
            motion_buffer_size: 0,
            max_request_length: 0,
            num_screens: 0,
            num_formats: 0,
            image_byte_order: ByteOrder::LSBFirst,
            bitmap_format_bit_order: BitOrder::LeastSignificant,
            bitmap_format_scanline_unit: 0,
            bitmap_format_scanline_pad: 0,
            min_keycode: 0 as char,
            max_keycode: 0 as char,
            vendor: String::new(),
            formats: vec![],
            screens: vec![]
        }
    }
}


////////////////////////////////////////
/// X TYPES
////////////////////////////////////////


#[derive(Debug)]
pub struct Screen {
    pub root: u32,
    pub default_colormap: u32,
    pub white_pixel: u32,
    pub black_pixel: u32,
    pub current_input_masks: u32, // TODO: This sets SETOfEVENT, but I don't know where the spec for this is
    pub width_in_pixels: u16,
    pub height_in_pixels: u16,
    pub width_in_millimeters: u16,
    pub height_in_millimeters: u16,
    pub min_installed_maps: u16,
    pub max_installed_maps: u16,
    pub root_visual: u32,
    pub backing_stores: ScreenBackingStores,
    pub save_unders: bool,
    pub root_depth: u8,
    pub num_depths: u8,
    pub depths: Vec<Depth>
}

#[derive(Debug)]
pub struct Window {
    pub depth: u8,
    pub wid: u32, // Window's ID
    pub parent: u32,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub class: WindowInputType,
    pub visual_id: u32,
    pub values: Vec<WindowValue>
}

impl Window {
    pub fn change_attrs(&mut self, client: &mut XClient, values: Vec<WindowValue>) {
        self.values = values;
        client.change_window_attributes(self.wid, &self.values);
    }

    pub fn set_attr(&mut self, client: &mut XClient, value: WindowValue) {
        let mut new_pos = self.values.len();

        for (i, val) in self.values.iter().enumerate() {
            if discriminant(val) == discriminant(&value) {
                new_pos = i;
                break;
            }
        }

        if new_pos == self.values.len() {
            self.values.push(value);
        } else {
            self.values.remove(new_pos);
            self.values.insert(new_pos, value);
        }

        client.change_window_attributes(self.wid, &self.values);
    }
}

#[derive(Debug)]
pub struct Depth {
    pub depth: u8,
    pub num_visuals: u16,
    pub visuals: Vec<Visual>
}

#[derive(Debug)]
pub struct Format {
    pub depth: u8,
    pub bits_per_pixel: u8,
    pub scanline_pad: u8
}

#[derive(Debug)]
pub struct Visual {
    pub id: u32,
    pub class: VisualType,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32
}

#[derive(Debug)]
pub struct Pixmap {
    pub depth: u8,
    pub pid: u32, // Pixmap's ID
    pub drawable: u32, // Window or Pixmap ID
    pub width: u16,
    pub height: u16
}

#[derive(Debug)]
pub struct GraphicsContext {
    pub cid: u32, // Graphic Context ID
    pub drawable: u32, // Window or Pixmap ID
    pub values: Vec<GraphicsContextValue>
}

impl Screen {
    pub fn empty() -> Screen {
        Screen {
            root: 0,
            default_colormap: 0,
            white_pixel: 0,
            black_pixel: 0,
            current_input_masks: 0,
            width_in_pixels: 0,
            height_in_pixels: 0,
            width_in_millimeters: 0,
            height_in_millimeters: 0,
            min_installed_maps: 0,
            max_installed_maps: 0,
            root_visual: 0,
            backing_stores: ScreenBackingStores::Never,
            save_unders: false,
            root_depth: 0,
            num_depths: 0,
            depths: vec![]
        }
    }
}

impl Depth {
    pub fn empty() -> Depth {
        Depth {
            depth: 0,
            num_visuals: 0,
            visuals: vec![]
        }
    }
}

impl Format {
    pub fn empty() -> Format {
        Format {
            depth: 0,
            bits_per_pixel: 0,
            scanline_pad: 0
        }
    }
}

impl Visual {
    pub fn empty() -> Visual {
        Visual {
            id: 0,
            class: VisualType::StaticGray,
            bits_per_rgb_value: 0,
            colormap_entries: 0,
            red_mask: 0,
            green_mask: 0,
            blue_mask: 0
        }
    }
}

#[derive(Debug)]
pub enum ServerError {
    Request { sequence_number: u16, minor_opcode: u16, major_opcode: u8 },
    Value { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_value: u32 },
    Window { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32, },
    Pixmap { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Atom { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_atom_id: u32 },
    Cursor { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Font { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Match { sequence_number: u16, minor_opcode: u16, major_opcode: u8 },
    Drawable { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Access { sequence_number: u16, minor_opcode: u16, major_opcode: u8 },
    Alloc { sequence_number: u16, minor_opcode: u16, major_opcode: u8 },
    Colormap { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    GContext { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    IDChoice { sequence_number: u16, minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Name { sequence_number: u16, minor_opcode: u16, major_opcode: u8 },
    Length { sequence_number: u16, minor_opcode: u16, major_opcode: u8 },
    Implementation { sequence_number: u16, minor_opcode: u16, major_opcode: u8 }
}

#[derive(Debug)]
pub enum ServerEvent {
    KeyPress {
        key_code: u8,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        same_screen: bool
    },
    KeyRelease {
        key_code: u8,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        same_screen: bool
    },
    ButtonPress {
        button: u8,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        same_screen: bool
    },
    ButtonRelease {
        button: u8,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        same_screen: bool
    },
    MotionNotify {
        detail: MotionNotifyType,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        same_screen: bool
    },
    EnterNotify {
        detail: NotifyType,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        mode: NotifyMode,
        same_screen: bool,
        focus: bool
    },
    LeaveNotify {
        detail: NotifyType,
        sequence_number: u16,
        time: u32,
        root: u32,
        event: u32,
        child: u32,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: Vec<KeyButton>,
        mode: NotifyMode,
        same_screen: bool,
        focus: bool
    },
    FocusIn {
        detail: FocusType,
        sequence_number: u16,
        event: u32,
        mode: FocusMode
    },
    FocusOut {
        detail: FocusType,
        sequence_number: u16,
        event: u32,
        mode: FocusMode
    },
    KeymapNotify {
        // TODO: Implement it
    },
    Expose {
        sequence_number: u16,
        window: u32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        count: u16
    },
    GraphicsExposure {
        sequence_number: u16,
        drawable: u32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        minor_opcode: u16,
        count: u16,
        major_opcode: u8
    },
    NoExposure {
        sequence_number: u16,
        drawable: u32,
        minor_opcode: u16,
        major_opcode: u8
    },
    VisibilityNotify {
        sequence_number: u16,
        window: u32,
        state: VisibilityState
    },
    CreateNotify {
        sequence_number: u16,
        parent: u32,
        window: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        override_redirect: bool
    },
    DestroyNotify {
        sequence_number: u16,
        event: u32,
        window: u32
    },
    UnmapNotify {
        sequence_number: u16,
        event: u32,
        window: u32,
        from_configure: bool
    },
    MapNotify {
        sequence_number: u16,
        event: u32,
        window: u32,
        override_redirect: bool
    },
    MapRequest {
        sequence_number: u16,
        parent: u32,
        window: u32
    },
    ReparentNotify {
        sequence_number: u16,
        event: u32,
        window: u32,
        parent: u32,
        x: i16,
        y: i16,
        override_redirect: bool
    },
    ConfigureNotify {
        sequence_number: u16,
        event: u32,
        window: u32,
        above_sibling: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        override_redirect: bool
    },
    ConfigureRequest {
        stack_mode: StackMode,
        sequence_number: u16,
        parent: u32,
        window: u32,
        sibling: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        values: Vec<ConfigureRequestValues>
    },
    GravityNotify {
        sequence_number: u16,
        event: u32,
        window: u32,
        x: i16,
        y: i16
    },
    ResizeRequest {
        sequence_number: u16,
        window: u32,
        width: u16,
        height: u16
    },
    CirculateNotify {
        sequence_number: u16,
        event: u32,
        window: u32,
        place: CirculatePlace
    },
    CirculateRequest {
        sequence_number: u16,
        parent: u32,
        window: u32,
        place: CirculatePlace
    },
    PropertyNotify {
        sequence_number: u16,
        window: u32,
        atom: u32,
        time: u32,
        state: PropertyState
    },
    SelectionClear {
        sequence_number: u16,
        time: u32,
        owner: u32,
        selection: u32
    },
    SelectionRequest {
        sequence_number: u16,
        time: u32,
        owner: u32,
        requestor: u32,
        selection: u32,
        target: u32,
        property: u32
    },
    SelectionNotify {
        sequence_number: u16,
        time: u32,
        requestor: u32,
        selection: u32,
        target: u32,
        property: u32
    },
    ColormapNotify {
        sequence_number: u16,
        window: u32,
        colormap: u32,
        new: bool,
        state: ColormapState
    },
    ClientMessage {
        format: u8,
        sequence_number: u16,
        window: u32,
        mtype: u32
    },
    MappingNotify {
        sequence_number: u16,
        request: MappingType,
        first_keycode: char,
        count: u8
    }
    // TODO: Continue at FocusIn
}

pub enum ServerResponse {
    Error(ServerError),
    Event(ServerEvent)
}


////////////////////////////////////////
/// VALUED
////////////////////////////////////////


#[derive(Debug)]
pub enum BitOrder {
    LeastSignificant,
    MostSignificant
}
impl Valued for BitOrder {
    fn val(&self) -> u32 {
        match self {
            &BitOrder::LeastSignificant => 0,
            &BitOrder::MostSignificant => 1
        }
    }
}

#[derive(Debug)]
pub enum ByteOrder {
    LSBFirst = 0,
    MSBFirst = 1
}
impl Valued for ByteOrder {
    fn val(&self) -> u32 {
        match self {
            &ByteOrder::LSBFirst => 0,
            &ByteOrder::MSBFirst => 1
        }
    }
}

#[derive(Debug)]
pub enum Event {
    KeyPress,
    KeyRelease,
    ButtonPress,
    ButtonRelease,
    EnterWindow,
    LeaveWindow,
    PointerMotion,
    PointerMotionHint,
    Button1Motion,
    Button2Motion,
    Button3Motion,
    Button4Motion,
    Button5Motion,
    ButtonMotion,
    KeymapState,
    Exposure,
    VisibilityChange,
    StructureNotify,
    ResizeRedirect,
    SubstructureNotify,
    SubstructureRedirect,
    FocusChange,
    PropertyChange,
    ColormapChange,
    OwnerGrabButton
}
impl Valued for Event {
    fn val(&self) -> u32 {
        match self {
            &Event::KeyPress => 0x00000001,
            &Event::KeyRelease => 0x00000002,
            &Event::ButtonPress => 0x00000004,
            &Event::ButtonRelease => 0x00000008,
            &Event::EnterWindow => 0x00000010,
            &Event::LeaveWindow => 0x00000020,
            &Event::PointerMotion => 0x00000040,
            &Event::PointerMotionHint => 0x00000080,
            &Event::Button1Motion => 0x00000100,
            &Event::Button2Motion => 0x00000200,
            &Event::Button3Motion => 0x00000400,
            &Event::Button4Motion => 0x00000800,
            &Event::Button5Motion => 0x00001000,
            &Event::ButtonMotion => 0x00002000,
            &Event::KeymapState => 0x00004000,
            &Event::Exposure => 0x00008000,
            &Event::VisibilityChange => 0x00010000,
            &Event::StructureNotify => 0x00020000,
            &Event::ResizeRedirect => 0x00040000,
            &Event::SubstructureNotify => 0x00080000,
            &Event::SubstructureRedirect => 0x00100000,
            &Event::FocusChange => 0x00200000,
            &Event::PropertyChange => 0x00400000,
            &Event::ColormapChange => 0x00800000,
            &Event::OwnerGrabButton => 0x01000000
        }
    }
}

#[derive(Debug)]
pub enum PointerEvent {
    ButtonPress,
    ButtonRelease,
    EnterWindow,
    LeaveWindow,
    PointerMotion,
    PointerMotionHint,
    Button1Motion,
    Button2Motion,
    Button3Motion,
    Button4Motion,
    Button5Motion,
    ButtonMotion,
    KeymapState
}
impl Valued for PointerEvent {
    fn val(&self) -> u32 {
        match self {
            &PointerEvent::ButtonPress => 0x00000004,
            &PointerEvent::ButtonRelease => 0x00000008,
            &PointerEvent::EnterWindow => 0x00000010,
            &PointerEvent::LeaveWindow => 0x00000020,
            &PointerEvent::PointerMotion => 0x00000040,
            &PointerEvent::PointerMotionHint => 0x00000080,
            &PointerEvent::Button1Motion => 0x00000100,
            &PointerEvent::Button2Motion => 0x00000200,
            &PointerEvent::Button3Motion => 0x00000400,
            &PointerEvent::Button4Motion => 0x00000800,
            &PointerEvent::Button5Motion => 0x00001000,
            &PointerEvent::ButtonMotion => 0x00002000,
            &PointerEvent::KeymapState => 0x00004000
        }
    }
}

#[derive(Debug)]
pub enum DeviceEvent {
    KeyPress,
    KeyRelease,
    ButtonPress,
    ButtonRelease,
    PointerMotion,
    Button1Motion,
    Button2Motion,
    Button3Motion,
    Button4Motion,
    Button5Motion,
    ButtonMotion
}
impl Valued for DeviceEvent {
    fn val(&self) -> u32 {
        match self {
            &DeviceEvent::KeyPress => 0x00000001,
            &DeviceEvent::KeyRelease => 0x00000002,
            &DeviceEvent::ButtonPress => 0x00000004,
            &DeviceEvent::ButtonRelease => 0x00000008,
            &DeviceEvent::PointerMotion => 0x00000040,
            &DeviceEvent::Button1Motion => 0x00000100,
            &DeviceEvent::Button2Motion => 0x00000200,
            &DeviceEvent::Button3Motion => 0x00000400,
            &DeviceEvent::Button4Motion => 0x00000800,
            &DeviceEvent::Button5Motion => 0x00001000,
            &DeviceEvent::ButtonMotion => 0x00002000
        }
    }
}

#[derive(Debug)]
pub enum ScreenBackingStores {
    Never,
    WhenMapped,
    Always
}
impl Valued for ScreenBackingStores {
    fn val(&self) -> u32 {
        match self {
            &ScreenBackingStores::Never => 0,
            &ScreenBackingStores::WhenMapped => 1,
            &ScreenBackingStores::Always => 2
        }
    }
}

#[derive(Debug)]
pub enum VisualType {
    StaticGray,
    GrayScale,
    StaticColor,
    PseudoColor,
    TrueColor,
    DirectColor
}
impl Valued for VisualType {
    fn val(&self) -> u32 {
        match self {
            &VisualType::StaticGray => 0,
            &VisualType::GrayScale => 1,
            &VisualType::StaticColor => 2,
            &VisualType::PseudoColor => 3,
            &VisualType::TrueColor => 4,
            &VisualType::DirectColor => 5
        }
    }
}

#[derive(Debug)]
pub enum WindowInputType {
    CopyFromParent,
    InputOutput,
    InputOnly
}
impl Valued for WindowInputType {
    fn val(&self) -> u32 {
        match self {
            &WindowInputType::CopyFromParent => 0,
            &WindowInputType::InputOutput => 1,
            &WindowInputType::InputOnly => 2
        }
    }
}

#[derive(Debug)]
pub enum WindowValueBackingStore {
    NotUseful,
    WhenMapped,
    Always
}
impl Valued for WindowValueBackingStore {
    fn val(&self) -> u32 {
        match self {
            &WindowValueBackingStore::NotUseful => 0,
            &WindowValueBackingStore::WhenMapped => 1,
            &WindowValueBackingStore::Always => 2
        }
    }
}

#[derive(Debug)]
pub enum BitGravity {
	Forget,
	Static,
	NorthWest,
	North,
	NorthEast,
	West,
	Center,
	East,
	SouthWest,
	South,
	SouthEast
}
impl Valued for BitGravity {
    fn val(&self) -> u32 {
        match self {
            &BitGravity::Forget => 0,
            &BitGravity::Static => 1,
            &BitGravity::NorthWest => 2,
            &BitGravity::North => 3,
            &BitGravity::NorthEast => 4,
            &BitGravity::West => 5,
            &BitGravity::Center => 6,
            &BitGravity::East => 7,
            &BitGravity::SouthWest => 8,
            &BitGravity::South => 9,
            &BitGravity::SouthEast => 10
        }
    }
}

#[derive(Debug)]
pub enum WindowGravity {
	Unmap,
	Static,
	NorthWest,
	North,
	NorthEast,
	West,
	Center,
	East,
	SouthWest,
	South,
	SouthEast
}
impl Valued for WindowGravity {
    fn val(&self) -> u32 {
        match self {
            &WindowGravity::Unmap => 0,
            &WindowGravity::Static => 1,
            &WindowGravity::NorthWest => 2,
            &WindowGravity::North => 3,
            &WindowGravity::NorthEast => 4,
            &WindowGravity::West => 5,
            &WindowGravity::Center => 6,
            &WindowGravity::East => 7,
            &WindowGravity::SouthWest => 8,
            &WindowGravity::South => 9,
            &WindowGravity::SouthEast => 10
        }
    }
}

#[derive(Debug)]
pub enum GCFunction {
	Clear,
	And,
	AndReverse,
	Copy,
	AndInverted,
	NoOp,
	Xor,
	Or,
	Nor,
	Equiv,
	Invert,
	OrReverse,
	CopyInverted,
	OrInverted,
	Nand,
	Set
}
impl Valued for GCFunction {
    fn val(&self) -> u32 {
        match self {
            &GCFunction::Clear => 0,
            &GCFunction::And => 1,
            &GCFunction::AndReverse => 2,
            &GCFunction::Copy => 3,
            &GCFunction::AndInverted => 4,
            &GCFunction::NoOp => 5,
            &GCFunction::Xor => 6,
            &GCFunction::Or => 7,
            &GCFunction::Nor => 8,
            &GCFunction::Equiv => 9,
            &GCFunction::Invert => 10,
            &GCFunction::OrReverse => 11,
            &GCFunction::CopyInverted => 12,
            &GCFunction::OrInverted => 13,
            &GCFunction::Nand => 14,
            &GCFunction::Set => 15
        }
    }
}

#[derive(Debug)]
pub enum GCLineStyle {
	Solid,
	OnOffDash,
	DoubleDash
}
impl Valued for GCLineStyle {
    fn val(&self) -> u32 {
        match self {
            &GCLineStyle::Solid => 0,
            &GCLineStyle::OnOffDash => 1,
            &GCLineStyle::DoubleDash => 2
        }
    }
}

#[derive(Debug)]
pub enum GCCapStyle {
	NotLast,
	Butt,
	Round,
	Projecting
}
impl Valued for GCCapStyle {
    fn val(&self) -> u32 {
        match self {
            &GCCapStyle::NotLast => 0,
            &GCCapStyle::Butt => 1,
            &GCCapStyle::Round => 2,
            &GCCapStyle::Projecting => 3
        }
    }
}

#[derive(Debug)]
pub enum GCJoinStyle {
	Miter,
	Round,
	Bevel
}
impl Valued for GCJoinStyle {
    fn val(&self) -> u32 {
        match self {
            &GCJoinStyle::Miter => 0,
            &GCJoinStyle::Round => 1,
            &GCJoinStyle::Bevel => 2
        }
    }
}

#[derive(Debug)]
pub enum GCFillStyle {
	Solid,
	Tiled,
	Stippled,
	OpaqueStippled
}
impl Valued for GCFillStyle {
    fn val(&self) -> u32 {
        match self {
            &GCFillStyle::Solid => 0,
            &GCFillStyle::Tiled => 1,
            &GCFillStyle::Stippled => 2,
            &GCFillStyle::OpaqueStippled => 3
        }
    }
}

#[derive(Debug)]
pub enum GCFillRule {
	EvenOdd,
	Winding
}
impl Valued for GCFillRule {
    fn val(&self) -> u32 {
        match self {
            &GCFillRule::EvenOdd => 0,
        	&GCFillRule::Winding => 1
        }
    }
}

#[derive(Debug)]
pub enum GCSubWindowMode {
	ClipByChildren = 0,
	IncludeInferiors = 1
}
impl Valued for GCSubWindowMode {
    fn val(&self) -> u32 {
        match self {
            &GCSubWindowMode::ClipByChildren => 0,
	        &GCSubWindowMode::IncludeInferiors => 1
        }
    }
}

#[derive(Debug)]
pub enum GCArcMode {
	Chord,
	PieSlice
}
impl Valued for GCArcMode {
    fn val(&self) -> u32 {
        match self {
            &GCArcMode::Chord => 0,
	        &GCArcMode::PieSlice => 1
        }
    }
}

#[derive(Debug)]
pub enum MotionNotifyType {
    Normal,
    Hint
}
impl MotionNotifyType {
    pub fn get(id: u8) -> Option<MotionNotifyType> {
        match id {
            0 => Some(MotionNotifyType::Normal),
            1 => Some(MotionNotifyType::Hint),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum NotifyType {
    Ancestor,
    Virtual,
    Inferior,
    Nonlinear,
    NonlinearVirtual
}
impl NotifyType {
    pub fn get(id: u8) -> Option<NotifyType> {
        match id {
            0 => Some(NotifyType::Ancestor),
            1 => Some(NotifyType::Virtual),
            2 => Some(NotifyType::Inferior),
            3 => Some(NotifyType::Nonlinear),
            4 => Some(NotifyType::NonlinearVirtual),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum FocusType {
    Ancestor,
    Virtual,
    Inferior,
    Nonlinear,
    NonlinearVirtual,
    Pointer,
    PointerRoot,
    None
}
impl FocusType {
    pub fn get(id: u8) -> Option<FocusType> {
        match id {
            0 => Some(FocusType::Ancestor),
            1 => Some(FocusType::Virtual),
            2 => Some(FocusType::Inferior),
            3 => Some(FocusType::Nonlinear),
            4 => Some(FocusType::NonlinearVirtual),
            5 => Some(FocusType::Pointer),
            6 => Some(FocusType::PointerRoot),
            7 => Some(FocusType::None),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum FocusMode {
    Normal,
    Grab,
    Ungrab,
    WhileGrabbed
}
impl FocusMode {
    pub fn get(id: u8) -> Option<FocusMode> {
        match id {
            0 => Some(FocusMode::Normal),
            1 => Some(FocusMode::Grab),
            2 => Some(FocusMode::Ungrab),
            3 => Some(FocusMode::WhileGrabbed),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum NotifyMode {
    Normal,
    Grab,
    Ungrab
}
impl NotifyMode {
    pub fn get(id: u8) -> Option<NotifyMode> {
        match id {
            0 => Some(NotifyMode::Normal),
            1 => Some(NotifyMode::Grab),
            2 => Some(NotifyMode::Ungrab),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum VisibilityState {
    Unobscured,
    PartiallyObscured,
    FullyObscured
}
impl VisibilityState {
    pub fn get(id: u8) -> Option<VisibilityState> {
        match id {
            0 => Some(VisibilityState::Unobscured),
            1 => Some(VisibilityState::PartiallyObscured),
            2 => Some(VisibilityState::FullyObscured),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum StackMode {
    Above,
    Below,
    TopIf,
    BottomIf,
    Opposite
}
impl StackMode {
    pub fn get(id: u8) -> Option<StackMode> {
        match id {
            0 => Some(StackMode::Above),
            1 => Some(StackMode::Below),
            2 => Some(StackMode::TopIf),
            3 => Some(StackMode::BottomIf),
            4 => Some(StackMode::Opposite),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum ConfigureRequestValues {
    X,
    Y,
    Width,
    Height,
    BorderWidth,
    Sibling,
    StackMode
}
impl ConfigureRequestValues {
    pub fn get(mask: u16) -> Vec<ConfigureRequestValues> {
        let mut v = vec![];
        
        if mask & 0x0001 == 0x0001 {
            v.push(ConfigureRequestValues::X);
        }
        if mask & 0x0002 == 0x0002 {
            v.push(ConfigureRequestValues::Y);
        }
        if mask & 0x0004 == 0x0004 {
            v.push(ConfigureRequestValues::Width);
        }
        if mask & 0x0008 == 0x0008 {
            v.push(ConfigureRequestValues::Height);
        }
        if mask & 0x0010 == 0x0010 {
            v.push(ConfigureRequestValues::BorderWidth);
        }
        if mask & 0x0020 == 0x0020 {
            v.push(ConfigureRequestValues::Sibling);
        }
        if mask & 0x0040 == 0x0040 {
            v.push(ConfigureRequestValues::StackMode);
        }
        
        return v;
    }
}

#[derive(Debug)]
pub enum CirculatePlace {
    Top,
    Bottom
}
impl CirculatePlace {
    pub fn get(id: u8) -> Option<CirculatePlace> {
        match id {
            0 => Some(CirculatePlace::Top),
            1 => Some(CirculatePlace::Bottom),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum PropertyState {
    NewValue,
    Deleted
}
impl PropertyState {
    pub fn get(id: u8) -> Option<PropertyState> {
        match id {
            0 => Some(PropertyState::NewValue),
            1 => Some(PropertyState::Deleted),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum ColormapState {
    Uninstalled,
    Installed
}
impl ColormapState {
    pub fn get(id: u8) -> Option<ColormapState> {
        match id {
            0 => Some(ColormapState::Uninstalled),
            1 => Some(ColormapState::Installed),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum MappingType {
    Modifier,
    Keyboard,
    Pointer
}
impl MappingType {
    pub fn get(id: u8) -> Option<MappingType> {
        match id {
            0 => Some(MappingType::Modifier),
            1 => Some(MappingType::Keyboard),
            2 => Some(MappingType::Pointer),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum KeyButton {
    Shift,
    Lock,
    Control,
    Mod1,
    Mod2,
    Mod3,
    Mod4,
    Mod5,
    Button1,
    Button2,
    Button3,
    Button4,
    Button5
}
impl KeyButton {
    pub fn get(mask: u16) -> Vec<KeyButton> {
        let mut v = vec![];
        
        if mask & 0x0001 == 0x0001 {
            v.push(KeyButton::Shift);
        }
        if mask & 0x0002 == 0x0002 {
            v.push(KeyButton::Lock);
        }
        if mask & 0x0004 == 0x0004 {
            v.push(KeyButton::Control);
        }
        if mask & 0x0008 == 0x0008 {
            v.push(KeyButton::Mod1);
        }
        if mask & 0x0010 == 0x0010 {
            v.push(KeyButton::Mod2);
        }
        if mask & 0x0020 == 0x0020 {
            v.push(KeyButton::Mod3);
        }
        if mask & 0x0040 == 0x0040 {
            v.push(KeyButton::Mod4);
        }
        if mask & 0x0080 == 0x0080 {
            v.push(KeyButton::Mod5);
        }
        if mask & 0x0100 == 0x0100 {
            v.push(KeyButton::Button1);
        }
        if mask & 0x0200 == 0x0200 {
            v.push(KeyButton::Button2);
        }
        if mask & 0x0400 == 0x0400 {
            v.push(KeyButton::Button3);
        }
        if mask & 0x0800 == 0x0800 {
            v.push(KeyButton::Button4);
        }
        if mask & 0x1000 == 0x1000 {
            v.push(KeyButton::Button5);
        }
        
        return v;
    }
}


////////////////////////////////////////
/// VALUES
////////////////////////////////////////


#[derive(Debug)]
pub enum WindowValue {
    BackgroundPixmap(u32),
    BackgroundPixel(u32),
    BorderPixmap(u32),
    BorderPixel(u32),
    BitGravity(BitGravity),
    WinGravity(WindowGravity),
    BackingStore(WindowValueBackingStore),
    BackingPlanes(u32),
    BackingPixel(u32),
    OverrideRedirect(bool),
    SaveUnder(bool),
    EventMask(u32),
    DoNotPropagateMask(u32),
    Colormap(u32),
    Cursor(u32)
}

#[derive(Debug)]
pub enum GraphicsContextValue {
    Function(GCFunction),
    PlaneMask(u32),
    Foreground(u32),
    Background(u32),
    LineWidth(u16),
    LineStyle(GCLineStyle),
    CapStyle(GCCapStyle),
    JoinStyle(GCJoinStyle),
    FillStyle(GCFillStyle),
    FillRule(GCFillRule),
    Tile(u32), // pixmap ID
    Stipple(u32), // pixmap ID
    TileStippleXOrigin(u16),
    TileStippleYOrigin(u16),
    Font(u32),
    SubWindowMode(GCSubWindowMode),
    GraphicsExposures(bool),
    ClipXOrigin(u16),
    ClipYOrigin(u16),
    ClipMask(u32), // pixmap ID
    DashOffset(u16),
    Dashes(u8),
    ArcMode(GCArcMode)
}


////////////////////////////////////////
/// VALUES METHODS
////////////////////////////////////////


impl Value for WindowValue {
    fn get_mask(&self) -> u32 {
        match self {
            &WindowValue::BackgroundPixmap(_) => 0x00000001,
            &WindowValue::BackgroundPixel(_) => 0x00000002,
            &WindowValue::BorderPixmap(_) => 0x00000004,
            &WindowValue::BorderPixel(_) => 0x00000008,
            &WindowValue::BitGravity(_) => 0x00000010,
            &WindowValue::WinGravity(_) => 0x00000020,
            &WindowValue::BackingStore(_) => 0x00000040,
            &WindowValue::BackingPlanes(_) => 0x00000080,
            &WindowValue::BackingPixel(_) => 0x00000100,
            &WindowValue::OverrideRedirect(_) => 0x00000200,
            &WindowValue::SaveUnder(_) => 0x00000400,
            &WindowValue::EventMask(_) => 0x00000800,
            &WindowValue::DoNotPropagateMask(_) => 0x00001000,
            &WindowValue::Colormap(_) => 0x00002000,
            &WindowValue::Cursor(_) => 0x00004000
        }
    }

    fn write<T: XBufferedWriter>(&self, client: &mut T) {
        match self {
            &WindowValue::BackgroundPixmap(val) => client.write_val_u32(val),
            &WindowValue::BackgroundPixel(val) => client.write_val_u32(val),
            &WindowValue::BorderPixmap(val) => client.write_val_u32(val),
            &WindowValue::BorderPixel(val) => client.write_val_u32(val),
            &WindowValue::BitGravity(ref val) => client.write_val(val.val()),
            &WindowValue::WinGravity(ref val) => client.write_val(val.val()),
            &WindowValue::BackingStore(ref val) => client.write_val(val.val()),
            &WindowValue::BackingPlanes(val) => client.write_val_u32(val),
            &WindowValue::BackingPixel(val) => client.write_val_u32(val),
            &WindowValue::OverrideRedirect(val) => client.write_val_bool(val),
            &WindowValue::SaveUnder(val) => client.write_val_bool(val),
            &WindowValue::EventMask(val) => client.write_val_u32(val),
            &WindowValue::DoNotPropagateMask(val) => client.write_val_u32(val),
            &WindowValue::Colormap(val) => client.write_val_u32(val),
            &WindowValue::Cursor(val) => client.write_val_u32(val)
        }
    }
}

impl Value for GraphicsContextValue {
    fn get_mask(&self) -> u32 {
        match self {
            &GraphicsContextValue::Function(_) => 0x00000001,
            &GraphicsContextValue::PlaneMask(_) => 0x00000002,
            &GraphicsContextValue::Foreground(_) => 0x00000004,
            &GraphicsContextValue::Background(_) => 0x00000008,
            &GraphicsContextValue::LineWidth(_) => 0x00000010,
            &GraphicsContextValue::LineStyle(_) => 0x00000020,
            &GraphicsContextValue::CapStyle(_) => 0x00000040,
            &GraphicsContextValue::JoinStyle(_) => 0x00000080,
            &GraphicsContextValue::FillStyle(_) => 0x00000100,
            &GraphicsContextValue::FillRule(_) => 0x00000200,
            &GraphicsContextValue::Tile(_) => 0x00000400,
            &GraphicsContextValue::Stipple(_) => 0x00000800,
            &GraphicsContextValue::TileStippleXOrigin(_) => 0x00001000,
            &GraphicsContextValue::TileStippleYOrigin(_) => 0x00002000,
            &GraphicsContextValue::Font(_) => 0x00004000,
            &GraphicsContextValue::SubWindowMode(_) => 0x00008000,
            &GraphicsContextValue::GraphicsExposures(_) => 0x00010000,
            &GraphicsContextValue::ClipXOrigin(_) => 0x00020000,
            &GraphicsContextValue::ClipYOrigin(_) => 0x00040000,
            &GraphicsContextValue::ClipMask(_) => 0x00080000,
            &GraphicsContextValue::DashOffset(_) => 0x00100000,
            &GraphicsContextValue::Dashes(_) => 0x00200000,
            &GraphicsContextValue::ArcMode(_) => 0x00400000
        }
    }

    fn write<T: XBufferedWriter>(&self, client: &mut T) {
        match self {
            &GraphicsContextValue::Function(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::PlaneMask(val) => client.write_val_u32(val),
            &GraphicsContextValue::Foreground(val) => client.write_val_u32(val),
            &GraphicsContextValue::Background(val) => client.write_val_u32(val),
            &GraphicsContextValue::LineWidth(val) => client.write_val_u16(val),
            &GraphicsContextValue::LineStyle(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::CapStyle(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::JoinStyle(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::FillStyle(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::FillRule(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::Tile(val) => client.write_val_u32(val),
            &GraphicsContextValue::Stipple(val) => client.write_val_u32(val),
            &GraphicsContextValue::TileStippleXOrigin(val) => client.write_val_u16(val),
            &GraphicsContextValue::TileStippleYOrigin(val) => client.write_val_u16(val),
            &GraphicsContextValue::Font(val) => client.write_val_u32(val),
            &GraphicsContextValue::SubWindowMode(ref val) => client.write_val(val.val()),
            &GraphicsContextValue::GraphicsExposures(val) => client.write_val_bool(val),
            &GraphicsContextValue::ClipXOrigin(val) => client.write_val_u16(val),
            &GraphicsContextValue::ClipYOrigin(val) => client.write_val_u16(val),
            &GraphicsContextValue::ClipMask(val) => client.write_val_u32(val),
            &GraphicsContextValue::DashOffset(val) => client.write_val_u16(val),
            &GraphicsContextValue::Dashes(val) => client.write_val_u8(val),
            &GraphicsContextValue::ArcMode(ref val) => client.write_val(val.val())
        };
    }
}
