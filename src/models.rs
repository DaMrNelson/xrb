use std::mem::discriminant;

use XClient;
use xreaderwriter::XBufferedWriter;

// Root trait for all values (ie GraphicsContextValue)
pub trait Value {
    fn get_mask(&self) -> u32;
    fn write(&self, client: &mut XClient);
}

//
//
//
//
////////////////////////////////////////
/// XRB TYPES
////////////////////////////////////////
//
//
//
//

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

#[derive(Debug)]
pub struct CharInfo {
    pub left_side_bearing: i16,
    pub right_side_bearingL: i16,
    pub character_width: i16,
    pub ascent: i16,
    pub descent: i16,
    pub attributes: u16
}

#[derive(Debug)]
pub struct FontProperty {
    pub name: u32,
    pub value: u32
}

#[derive(Debug)]
pub enum ServerError {
    Request { minor_opcode: u16, major_opcode: u8 },
    Value { minor_opcode: u16, major_opcode: u8, bad_value: u32 },
    Window { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32, },
    Pixmap { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Atom { minor_opcode: u16, major_opcode: u8, bad_atom_id: u32 },
    Cursor { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Font { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Match { minor_opcode: u16, major_opcode: u8 },
    Drawable { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Access { minor_opcode: u16, major_opcode: u8 },
    Alloc { minor_opcode: u16, major_opcode: u8 },
    Colormap { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    GContext { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    IDChoice { minor_opcode: u16, major_opcode: u8, bad_resource_id: u32 },
    Name { minor_opcode: u16, major_opcode: u8 },
    Length { minor_opcode: u16, major_opcode: u8 },
    Implementation { minor_opcode: u16, major_opcode: u8 }
}

#[derive(Debug)]
pub enum ServerReplyType { // Used to specify a ServerReply type without creating the entire object
    GetWindowAttributes,
    GetGeometry,
    QueryTree,
    InternAtom,
    GetAtomName,
    GetProperty,
    ListProperties,
    GetSelectionOwner,
    GrabPointer,
    GrabKeyboard,
    QueryPointer,
    GetMotionEvents,
    TranslateCoordinates,
    GetInputFocus,
    QueryKeymap,
    QueryFont,
    QueryTextExtents,
    ListFonts,
    ListFontsWithInfo, // Note: One request will generate multiple replies here. The info specifies how to determine this
    GetFontPath,
    GetImage,
    ListInstalledColormaps,
    AllocColor,
    AllocNamedColor,
    AllocColorCells,
    AllocColorPlanes,
    QueryColors,
    LookupColor,
    QueryBestSize,
    QueryExtension,
    ListExtensions,
    GetKeyboardMapping,
    GetKeyboardControl,
    GetPointerControl,
    GetScreenSaver,
    ListHosts,
    SetPointerMapping,
    GetPointerMapping,
    SetModifierMapping,
    GetModifierMapping,
    None
}

#[derive(Debug)]
pub enum ServerReply {
    GetWindowAttributes {
        backing_store: WindowBackingStore,
        visual: u32,
        class: WindowInputType,
        bit_gravity: BitGravity,
        window_gravity: WindowGravity,
        backing_planes: u32,
        backing_pixel: u32,
        save_under: bool,
        map_is_installed: bool,
        map_state: MapState,
        override_redirect: bool,
        colormap: u32,
        all_event_masks: u32,
        your_event_mask: u32,
        do_not_propagate_mask: u16
    },
    GetGeometry {
        root: u32,
        depth: u8,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16
    },
    QueryTree {
        root: u32,
        parent: u32, // 0 = None
        wids: Vec<u32>
    },
    InternAtom {
        atom: u32 // 0 = None
    },
    GetAtomName {
        name: String
    },
    GetProperty {
        vtype: u32, // atom, 0 = None
        value: Vec<u8>
    },
    ListProperties {
        atoms: Vec<u32>
    },
    GetSelectionOwner {
        wid: u32
    },
    GrabPointer {
        status: GrabStatus
    },
    GrabKeyboard {
        status: GrabStatus
    },
    QueryPointer {
        root: u32,
        child: u32, // 0 = none
        root_x: i16,
        root_y: i16,
        win_x: i16,
        win_y: i16,
        key_buttons: Vec<KeyButton>,
        same_screen: bool
    },
    GetMotionEvents {
        events: Vec<TimeCoordinate>
    },
    TranslateCoordinates {
        child: u32, // 0 = none
        dst_x: i16,
        dst_y: i16,
        same_screen: bool
    },
    GetInputFocus {
        wid: u32, // 0 = none, 1 = pointer root
        revert_to: InputFocusRevert
    },
    QueryKeymap {
        keys: Vec<char>
    },
    QueryFont {
        // TODO
    },
    QueryTextExtents {
        font_ascent: i16,
        font_descent: i16,
        overall_ascent: i16,
        overall_descent: i16,
        overall_width: i32,
        overall_left: i32,
        overall_right: i32,
        draw_direction: FontDrawDirection
    },
    ListFonts {
        names: Vec<String>
    },
    ListFontsWithInfoEntry { // Ended by ListFontsWithInfoEnd
        min_bounds: CharInfo,
        max_bounds: CharInfo,
        min_char: u16,
        max_char: u16,
        default_char: u16,
        draw_direction: FontDrawDirection,
        min_byte: u8,
        max_byte: u8,
        all_chars_exist: bool,
        font_ascent: i16,
        font_descent: i16,
        replies_hint: u32,
        properties: Vec<FontProperty>,
        name: String
    },
    ListFontsWithInfoEnd, // End marker for ListFontsWithInfoEntry
    GetFontPath {
        path: Vec<String>
    },
    GetImage {
        visual: u32, // 0 = none
        depth: u8,
        data: Vec<u8>
    },
    ListInstalledColormaps {
        cmids: Vec<u32>
    },
    AllocColor {
        pixel: u32,
        red: u16,
        green: u16,
        blue: u16
    },
    AllocNamedColor {
        pixel: u32,
        exact_red: u16,
        exact_green: u16,
        exact_blue: u16,
        visual_red: u16,
        visual_green: u16,
        visual_blue: u16
    },
    AllocColorCells {
        pixels: Vec<u32>,
        masks: Vec<u32>
    },
    AllocColorPlanes {
        pixels: Vec<u32>,
        red_mask: u32,
        green_mask: u32,
        blue_mask: u32
    },
    QueryColors {
        colors: Vec<Color>
    },
    LookupColor {
        exact_red: u16,
        exact_green: u16,
        exact_blue: u16,
        visual_red: u16,
        visual_green: u16,
        visual_blue: u16
    },
    QueryBestSize {
        width: u16,
        height: u16
    },
    QueryExtension {
        present: bool,
        major_opcode: u8,
        first_event: u8,
        first_error: u8
    },
    ListExtensions {
        names: Vec<String>
    },
    GetKeyboardMapping {
        // TODO
    },
    GetKeyboardControl {
        global_auto_repeat: KeyboardControlAutoRepeatMode,
        led_mask: u32,
        key_click_percent: u8,
        bell_percent: u8,
        bell_pitch: u16,
        bell_duration: u16,
        auto_repeats: Vec<u8>
    },
    GetPointerControl {
        acceleration_numerator: u16,
        acceleration_denominator: u16,
        threshold: u16
    },
    GetScreenSaver {
        timeout: u16,
        interval: u16,
        prefer_blanking: bool,
        allow_exposures: bool
    },
    ListHosts {
        enabled: bool,
        hosts: Vec<Host>
    },
    SetPointerMapping {
        success: bool
    },
    GetPointerMapping {
        map: Vec<u8>
    },
    SetModifierMapping {
        status: SetModifierMappingStatus
    },
    GetModifierMapping {
        key_codes: Vec<char>
    }
}

#[derive(Debug)]
pub enum ServerEvent {
    KeyPress {
        key_code: u8,
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
        event: u32,
        mode: FocusMode
    },
    FocusOut {
        detail: FocusType,
        event: u32,
        mode: FocusMode
    },
    KeymapNotify {
        // TODO: Implement it
    },
    Expose {
        window: u32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        count: u16
    },
    GraphicsExposure {
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
        drawable: u32,
        minor_opcode: u16,
        major_opcode: u8
    },
    VisibilityNotify {
        window: u32,
        state: VisibilityState
    },
    CreateNotify {
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
        event: u32,
        window: u32
    },
    UnmapNotify {
        event: u32,
        window: u32,
        from_configure: bool
    },
    MapNotify {
        event: u32,
        window: u32,
        override_redirect: bool
    },
    MapRequest {
        parent: u32,
        window: u32
    },
    ReparentNotify {
        event: u32,
        window: u32,
        parent: u32,
        x: i16,
        y: i16,
        override_redirect: bool
    },
    ConfigureNotify {
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
        event: u32,
        window: u32,
        x: i16,
        y: i16
    },
    ResizeRequest {
        window: u32,
        width: u16,
        height: u16
    },
    CirculateNotify {
        event: u32,
        window: u32,
        place: CirculatePlace
    },
    CirculateRequest {
        parent: u32,
        window: u32,
        place: CirculatePlace
    },
    PropertyNotify {
        window: u32,
        atom: u32,
        time: u32,
        state: PropertyState
    },
    SelectionClear {
        time: u32,
        owner: u32,
        selection: u32
    },
    SelectionRequest {
        time: u32,
        owner: u32,
        requestor: u32,
        selection: u32,
        target: u32,
        property: u32
    },
    SelectionNotify {
        time: u32,
        requestor: u32,
        selection: u32,
        target: u32,
        property: u32
    },
    ColormapNotify {
        window: u32,
        colormap: u32,
        new: bool,
        state: ColormapState
    },
    ClientMessage {
        format: u8,
        window: u32,
        mtype: u32,
        data: [u8; 20]
    },
    MappingNotify {
        request: MappingType,
        first_keycode: char,
        count: u8
    }
}

#[derive(Debug)]
pub enum ServerResponse {
    Error(ServerError, u16),
    Reply(ServerReply, u16),
    Event(ServerEvent, u16)
}

//
//
//
//
////////////////////////////////////////
/// X TYPES
////////////////////////////////////////
//
//
//
//

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

pub trait Drawable {
    fn get_drawable(&self) -> u32;

    /** Tells the X server to [TODO] */
    fn draw_point(&self, client: &mut XClient, gcid: u32, point: Point, mode: &CoordinateMode) {
        client.poly_point(self.get_drawable(), gcid, &vec![point], mode)
    }

    /** Tells the X server to [TODO] */
    fn draw_points(&self, client: &mut XClient, gcid: u32, points: &Vec<Point>, mode: &CoordinateMode) {
        client.poly_point(self.get_drawable(), gcid, points, mode)
    }

    /** Tells the X Server to [TODO] */
    fn draw_line(&self, client: &mut XClient, gcid: u32, point: Point, mode: &CoordinateMode) {
        client.poly_line(self.get_drawable(), gcid, &vec![point], mode)
    }

    /** Tells the X Server to [TODO] */
    fn draw_lines(&self, client: &mut XClient, gcid: u32, points: &Vec<Point>, mode: &CoordinateMode) {
        client.poly_line(self.get_drawable(), gcid, points, mode)
    }

    /** Tells the X Server to [TODO] */
    fn draw_seg(&self, client: &mut XClient, gcid: u32, segment: Segment) {
        client.poly_segment(self.get_drawable(), gcid, &vec![segment])
    }

    /** Tells the X Server to [TODO] */
    fn draw_segs(&self, client: &mut XClient, gcid: u32, segments: &Vec<Segment>) {
        client.poly_segment(self.get_drawable(), gcid, segments)
    }

    /** Tells the X Server to [TODO] */
    fn draw_rect(&self, client: &mut XClient, gcid: u32, rectangle: Rectangle) {
        client.poly_rectangle(self.get_drawable(), gcid, &vec![rectangle])
    }

    /** Tells the X Server to [TODO] */
    fn draw_rects(&self, client: &mut XClient, gcid: u32, rectangles: &Vec<Rectangle>) {
        client.poly_rectangle(self.get_drawable(), gcid, rectangles)
    }

    /** Tells the X Server to [TODO] */
    fn draw_arc(&self, client: &mut XClient, gcid: u32, arc: Arc) {
        client.poly_arc(self.get_drawable(), gcid, &vec![arc])
    }

    /** Tells the X Server to [TODO] */
    fn draw_arcs(&self, client: &mut XClient, gcid: u32, arcs: &Vec<Arc>) {
        client.poly_arc(self.get_drawable(), gcid, arcs)
    }

    /** Tells the X Server to [TODO] */
    fn fill_poly(&self, client: &mut XClient, gcid: u32, point: Point, shape: &PolyShape, mode: &CoordinateMode) {
        client.fill_poly(self.get_drawable(), gcid, &vec![point], shape, mode)
    }

    /** Tells the X Server to [TODO] */
    fn fill_polys(&self, client: &mut XClient, gcid: u32, points: &Vec<Point>, shape: &PolyShape, mode: &CoordinateMode) {
        client.fill_poly(self.get_drawable(), gcid, points, shape, mode)
    }

    /** Tells the X Server to [TODO] */
    fn fill_rect(&self, client: &mut XClient, gcid: u32, rectangle: Rectangle) {
        client.poly_fill_rectangle(self.get_drawable(), gcid, &vec![rectangle])
    }

    /** Tells the X Server to [TODO] */
    fn fill_rects(&self, client: &mut XClient, gcid: u32, rectangles: &Vec<Rectangle>) {
        client.poly_fill_rectangle(self.get_drawable(), gcid, rectangles)
    }

    /** Tells the X Server to [TODO] */
    fn poly_fill_arc(&self, client: &mut XClient, gcid: u32, arc: Arc) {
        client.poly_fill_arc(self.get_drawable(), gcid, &vec![arc])
    }

    /** Tells the X Server to [TODO] */
    fn poly_fill_arcs(&self, client: &mut XClient, gcid: u32, arcs: &Vec<Arc>) {
        client.poly_fill_arc(self.get_drawable(), gcid, arcs)
    }

    /** Tells the X Server to [TODO] */
    fn put_image(&self, client: &mut XClient, gcid: u32, data: &Vec<u8>, width: u16, height: u16, x: i16, y: i16, left_pad: u8, depth: u8, format: &ImageFormat) {
        client.put_image(self.get_drawable(), gcid, data, width, height, x, y, left_pad, depth, format)
    }

    /**
     * Tells the X Server to [TODO]
     * `format` may only be ImageFormat::XYPixmap or ImageFormat::ZPixmap
     */
    fn get_image(&self, client: &mut XClient, x: i16, y: i16, width: u16, height: u16, plane_mask: u32, format: &ImageFormat) -> u16 {
        client.get_image(self.get_drawable(), x, y, width, height, plane_mask, format)
        // TODO: Sync get_image
    }

    /**
     * Tells the X Server to [TODO]
     * `texts` is TextItem8Text or TextItem8Font
     * A TextItem8Text entry in `texts` must be 254 or less characters
     */
    fn text8<T: TextItem8>(&self, client: &mut XClient, gcid: u32, x: i16, y: i16, text: T) {
        client.poly_text8(self.get_drawable(), gcid, x, y, &vec![text])
    }
    
    /**
     * Tells the X Server to [TODO]
     * `texts` is TextItem8Text or TextItem8Font
     * A TextItem8Text entry in `texts` must be 254 or less characters
     */
    fn text8s<T: TextItem8>(&self, client: &mut XClient, gcid: u32, x: i16, y: i16, texts: &Vec<T>) {
        client.poly_text8(self.get_drawable(), gcid, x, y, texts)
    }

    /**
     * Tells the X Server to [TODO]
     * `texts` is TextItem16Text or TextItem16Font
     * A TextItem16Text entry in `texts` must be 254 or less characters
     */
    fn text16<T: TextItem16>(&self, client: &mut XClient, gcid: u32, x: i16, y: i16, text: T) {
        client.poly_text16(self.get_drawable(), gcid, x, y, &vec![text])
    }

    /**
     * Tells the X Server to [TODO]
     * `texts` is TextItem16Text or TextItem16Font
     * A TextItem16Text entry in `texts` must be 254 or less characters
     */
    fn text16s<T: TextItem16>(&self, client: &mut XClient, gcid: u32, x: i16, y: i16, texts: &Vec<T>) {
        client.poly_text16(self.get_drawable(), gcid, x, y, texts)
    }

    /**
     * Tells the X Server to [TODO] 
     * `text` must be 255 or less characters
     */
    fn image_text8(&self, client: &mut XClient, gcid: u32, text: &str, x: i16, y: i16) {
        client.image_text8(self.get_drawable(), gcid, text, x, y)
    }

    /**
     * Tells the X Server to [TODO]
     * `text` must have 255 or less elements
     */
    fn image_text16(&self, client: &mut XClient, gcid: u32, text: &Vec<u16>, x: i16, y: i16) {
        client.image_text16(self.get_drawable(), gcid, text, x, y)
    }

    /**
     * Tells the X server to [TODO]
     * `text` must have 255 or less elements
     */
    fn get_geometry(&self, client: &mut XClient) -> u16 {
        client.get_geometry(self.get_drawable())
        // TODO: sync get_gemometry
    }
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
    /**
     * Gets a window and its information from the server.
     * This function will block until the X server replies.
     */
    pub fn get_sync(client: &mut XClient, wid: u32) -> Result<Window, ServerError> {
        let seq1 = client.get_geometry(wid);
        let seq2 = client.get_window_attributes(wid);
        let (depth, root, x, y, width, height, border_width) = match client.wait_for_response(seq1) {
            ServerResponse::Error(err, _) => return Err(err),
            ServerResponse::Reply(reply, _) => match reply {
                ServerReply::GetGeometry { depth, root, x, y, width, height, border_width }
                    => (depth, root, x, y, width, height, border_width),
                _ => unreachable!()
            },
            _ => unreachable!()
        };
        let (backing_store, visual, class, bit_gravity, window_gravity, backing_planes, backing_pixel, save_under, map_is_installed, map_state, override_redirect, colormap, all_event_masks, your_event_mask, do_not_propagate_mask) = match client.wait_for_response(seq2) {
            ServerResponse::Error(err, _) => return Err(err),
            ServerResponse::Reply(reply, _) => match reply {
                ServerReply::GetWindowAttributes { backing_store, visual, class, bit_gravity, window_gravity, backing_planes, backing_pixel, save_under, map_is_installed, map_state, override_redirect, colormap, all_event_masks, your_event_mask, do_not_propagate_mask }
                    => (backing_store, visual, class, bit_gravity, window_gravity, backing_planes, backing_pixel, save_under, map_is_installed, map_state, override_redirect, colormap, all_event_masks, your_event_mask, do_not_propagate_mask),
                _ => unreachable!()
            },
            _ => unreachable!()
        };
        Ok(Window {
            depth,
            wid,
            parent: root,
            x,
            y,
            width,
            height,
            border_width,
            class,
            visual_id: visual,
            values: vec![
                //WindowValue::BackgroundPixmap(0), // TODO: Get this
                //WindowValue::BackgroundPixel(0), // TODO: Get this
                //WindowValue::BorderPixmap(0), // TODO: Get this
                //WindowValue::BorderPixel(0), // TODO: Get this
                WindowValue::BitGravity(bit_gravity),
                WindowValue::WinGravity(window_gravity),
                WindowValue::BackingStore(backing_store),
                WindowValue::BackingPlanes(backing_planes),
                WindowValue::BackingPixel(backing_pixel),
                WindowValue::OverrideRedirect(override_redirect),
                WindowValue::SaveUnder(save_under),
                WindowValue::EventMask(your_event_mask),
                WindowValue::DoNotPropagateMask(do_not_propagate_mask as u32),
                WindowValue::Colormap(colormap),
                //WindowValue::Cursor(0) // TODO: Get this
            ]
        })
    }

    pub fn change(&mut self, client: &mut XClient, values: Vec<WindowValue>) {
        self.values = values;
        client.change_window_attributes(self.wid, &self.values);
    }

    pub fn set(&mut self, client: &mut XClient, value: WindowValue) {
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

impl Drawable for Window {
    #[inline(always)]
    fn get_drawable(&self) -> u32 {
        self.wid
    }
}

#[derive(Debug)]
pub struct Depth {
    pub depth: u8,
    pub num_visuals: u16,
    pub visuals: Vec<Visual>
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

#[derive(Debug)]
pub struct Format {
    pub depth: u8,
    pub bits_per_pixel: u8,
    pub scanline_pad: u8
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
pub struct Pixmap {
    pub depth: u8,
    pub pid: u32, // Pixmap's ID
    pub drawable: u32, // Window or Pixmap ID
    pub width: u16,
    pub height: u16
}

impl Drawable for Pixmap {
    #[inline(always)]
    fn get_drawable(&self) -> u32 {
        self.pid
    }
}

#[derive(Debug)]
pub struct GraphicsContext {
    pub gcid: u32, // Graphic Context ID
    pub drawable: u32, // Window or Pixmap ID
    pub values: Vec<GraphicsContextValue>
}

impl GraphicsContext {
    pub fn change(&mut self, client: &mut XClient, values: Vec<GraphicsContextValue>) {
        self.values = values;
        client.change_gc(self.gcid, &self.values);
    }

    pub fn set(&mut self, client: &mut XClient, value: GraphicsContextValue) {
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

        client.change_gc(self.gcid, &self.values);
    }

    pub fn free(&self, client: &mut XClient) {
        client.free_gc(self.gcid);
    }

    pub fn set_bg(&mut self, client: &mut XClient, color: &Color) {
        self.set_bg_raw(client, color.num());
    }

    pub fn set_bg_raw(&mut self, client: &mut XClient, color: u32) {
        self.set(client, GraphicsContextValue::Background(color))
    }

    pub fn set_fg(&mut self, client: &mut XClient, color: &Color) {
        self.set_fg_raw(client, color.num());
    }

    pub fn set_fg_raw(&mut self, client: &mut XClient, color: u32) {
        self.set(client, GraphicsContextValue::Foreground(color))
    }
}

#[derive(Debug)]
pub struct Point {
    pub x: i16,
    pub y: i16
}
impl Point {
    pub fn write(&self, client: &mut XClient) {
        client.write_i16(self.x);
        client.write_i16(self.y);
    }
}

#[derive(Debug)]
pub struct Rectangle {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16
}
impl Rectangle {
    pub fn write(&self, client: &mut XClient) {
        client.write_i16(self.x);
        client.write_i16(self.y);
        client.write_u16(self.width);
        client.write_u16(self.height);
    }
}

#[derive(Debug)]
pub struct Arc {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub angle1: i16,
    pub angle2: i16
}
impl Arc {
    pub fn write(&self, client: &mut XClient) {
        client.write_i16(self.x);
        client.write_i16(self.y);
        client.write_u16(self.width);
        client.write_u16(self.height);
        client.write_i16(self.angle1);
        client.write_i16(self.angle2);
    }
}

#[derive(Debug)]
pub struct Segment {
    x1: i16,
    y1: i16,
    x2: i16,
    y2: i16
}
impl Segment {
    pub fn write(&self, client: &mut XClient) {
        client.write_i16(self.x1);
        client.write_i16(self.y1);
        client.write_i16(self.x2);
        client.write_i16(self.y2);
    }
}

pub trait TextItem8 {
    fn len(&self) -> usize;
    fn write(&self, client: &mut XClient);
}

#[derive(Debug)]
pub struct TextItem8Text {
    delta: i8,
    text: str
}
impl TextItem8 for TextItem8Text {
    fn len(&self) -> usize {
        2 + self.text.len()
    }

    fn write(&self, client: &mut XClient) {
        client.write_u8(self.text.len() as u8);
        client.write_i8(self.delta);
        client.write_str(&self.text);
    }
}

#[derive(Debug)]
pub struct TextItem8Font {
    bytes: [u8; 4]
}
impl TextItem8 for TextItem8Font {
    fn len(&self) -> usize {
        4
    }

    fn write(&self, client: &mut XClient) {
        client.write_raw(&self.bytes);
    }
}

pub trait TextItem16 {
    fn len(&self) -> usize;
    fn write(&self, client: &mut XClient);
}

#[derive(Debug)]
pub struct TextItem16Text {
    delta: i8,
    text: Vec<u16>
}
impl TextItem16 for TextItem16Text {
    fn len(&self) -> usize {
        2 + self.text.len() * 2
    }

    fn write(&self, client: &mut XClient) {
        client.write_u8(self.text.len() as u8);
        client.write_i8(self.delta);

        for c in &self.text {
            client.write_u16(*c);
        }
    }
}

#[derive(Debug)]
pub struct TextItem16Font {
    bytes: [u8; 4]
}
impl TextItem16 for TextItem16Font {
    fn len(&self) -> usize {
        5
    }

    fn write(&self, client: &mut XClient) {
        client.write_u8(255);
        client.write_raw(&self.bytes);
    }
}

#[derive(Debug)]
pub struct ColorItem {
    pub pixel: u32,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub do_red: bool, // TODO: Is this supposed to be one of those or a mask of them?
    pub do_green: bool,
    pub do_blue: bool
}
impl ColorItem {
    pub fn write(&self, client: &mut XClient) {
        let mut mask = 0x00;
        if self.do_red {
            mask |= 0x01;
        }
        if self.do_green {
            mask |= 0x02;
        }
        if self.do_blue {
            mask |= 0x04;
        }

        client.write_u32(self.pixel);
        client.write_u16(self.red);
        client.write_u16(self.green);
        client.write_u16(self.blue);
        client.write_u8(mask);
        client.write_pad(1);
    }
}

#[derive(Debug)]
pub struct TimeCoordinate {
    pub time: u32,
    pub x: i16,
    pub y: i16
}

#[derive(Debug)]
pub struct Color {
    pub red: u16,
    pub green: u16,
    pub blue: u16
}

impl Color {
    pub fn from_rgb(red: u16, green: u16, blue: u16) -> Color {
        Color { red, green, blue }
    }

    pub fn from_num(num: u32) -> Color {
        Color {
            red: ((num & 0xFF0000) >> 16) as u16,
            green: ((num & 0x00FF00) >> 8) as u16,
            blue: (num & 0x0000FF) as u16
        }
    }

    pub fn num(&self) -> u32 {
        return ((self.red as u32) << 16) + ((self.green as u32) << 8) + (self.blue as u32)
    }
}

#[derive(Debug)]
pub struct Host {
    pub family: HostFamily,
    pub address: Vec<u8>
}

//
//
//
//
////////////////////////////////////////
/// VALUED
////////////////////////////////////////
//
//
//
//

#[derive(Debug)]
pub enum BitOrder {
    LeastSignificant,
    MostSignificant
}
impl BitOrder {
    pub fn val(&self) -> u32 {
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
impl ByteOrder {
    pub fn val(&self) -> u32 {
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
impl Event {
    pub fn val(&self) -> u32 {
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
impl PointerEvent {
    pub fn val(&self) -> u16 {
        match self {
            &PointerEvent::ButtonPress => 0x0004,
            &PointerEvent::ButtonRelease => 0x0008,
            &PointerEvent::EnterWindow => 0x0010,
            &PointerEvent::LeaveWindow => 0x0020,
            &PointerEvent::PointerMotion => 0x0040,
            &PointerEvent::PointerMotionHint => 0x0080,
            &PointerEvent::Button1Motion => 0x0100,
            &PointerEvent::Button2Motion => 0x0200,
            &PointerEvent::Button3Motion => 0x0400,
            &PointerEvent::Button4Motion => 0x0800,
            &PointerEvent::Button5Motion => 0x1000,
            &PointerEvent::ButtonMotion => 0x2000,
            &PointerEvent::KeymapState => 0x4000
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
impl DeviceEvent {
    pub fn val(&self) -> u16 {
        match self {
            &DeviceEvent::KeyPress => 0x0001,
            &DeviceEvent::KeyRelease => 0x0002,
            &DeviceEvent::ButtonPress => 0x0004,
            &DeviceEvent::ButtonRelease => 0x0008,
            &DeviceEvent::PointerMotion => 0x0040,
            &DeviceEvent::Button1Motion => 0x0100,
            &DeviceEvent::Button2Motion => 0x0200,
            &DeviceEvent::Button3Motion => 0x0400,
            &DeviceEvent::Button4Motion => 0x0800,
            &DeviceEvent::Button5Motion => 0x1000,
            &DeviceEvent::ButtonMotion => 0x2000
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

    pub fn val(&self) -> u16 {
        match self {
            &KeyButton::Shift => 0x0001,
            &KeyButton::Lock => 0x0002,
            &KeyButton::Control => 0x0004,
            &KeyButton::Mod1 => 0x0008,
            &KeyButton::Mod2 => 0x0010,
            &KeyButton::Mod3 => 0x0020,
            &KeyButton::Mod4 => 0x0040,
            &KeyButton::Mod5 => 0x0080,
            &KeyButton::Button1 => 0x0100,
            &KeyButton::Button2 => 0x0200,
            &KeyButton::Button3 => 0x0400,
            &KeyButton::Button4 => 0x0800,
            &KeyButton::Button5 => 0x1000
        }
    }
}

#[derive(Debug)]
pub enum Key {
    Shift,
    Lock,
    Control,
    Mod1,
    Mod2,
    Mod3,
    Mod4,
    Mod5
}
impl Key {
    pub fn get(mask: u16) -> Vec<Key> {
        let mut v = vec![];
        
        if mask & 0x0001 == 0x0001 {
            v.push(Key::Shift);
        }
        if mask & 0x0002 == 0x0002 {
            v.push(Key::Lock);
        }
        if mask & 0x0004 == 0x0004 {
            v.push(Key::Control);
        }
        if mask & 0x0008 == 0x0008 {
            v.push(Key::Mod1);
        }
        if mask & 0x0010 == 0x0010 {
            v.push(Key::Mod2);
        }
        if mask & 0x0020 == 0x0020 {
            v.push(Key::Mod3);
        }
        if mask & 0x0040 == 0x0040 {
            v.push(Key::Mod4);
        }
        if mask & 0x0080 == 0x0080 {
            v.push(Key::Mod5);
        }
        
        return v;
    }

    pub fn val(&self) -> u16 {
        match self {
            &Key::Shift => 0x0001,
            &Key::Lock => 0x0002,
            &Key::Control => 0x0004,
            &Key::Mod1 => 0x0008,
            &Key::Mod2 => 0x0010,
            &Key::Mod3 => 0x0020,
            &Key::Mod4 => 0x0040,
            &Key::Mod5 => 0x0080
        }
    }
}

#[derive(Debug)]
pub enum ScreenBackingStores {
    Never,
    WhenMapped,
    Always
}
impl ScreenBackingStores {
    pub fn val(&self) -> u32 {
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
impl VisualType {
    pub fn val(&self) -> u32 {
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
impl WindowInputType {
    pub fn get(id: u16) -> Option<WindowInputType> {
        match id {
            0 => Some(WindowInputType::CopyFromParent),
            1 => Some(WindowInputType::InputOutput),
            2 => Some(WindowInputType::InputOnly),
            _ => None
        }
    }

    pub fn val(&self) -> u32 {
        match self {
            &WindowInputType::CopyFromParent => 0,
            &WindowInputType::InputOutput => 1,
            &WindowInputType::InputOnly => 2
        }
    }
}

#[derive(Debug)]
pub enum WindowBackingStore {
    NotUseful,
    WhenMapped,
    Always
}
impl WindowBackingStore {
    pub fn get(id: u8) -> Option<WindowBackingStore> {
        match id {
            0 => Some(WindowBackingStore::NotUseful),
            1 => Some(WindowBackingStore::WhenMapped),
            2 => Some(WindowBackingStore::Always),
            _ => None
        }
    }

    pub fn val(&self) -> u32 {
        match self {
            &WindowBackingStore::NotUseful => 0,
            &WindowBackingStore::WhenMapped => 1,
            &WindowBackingStore::Always => 2
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
impl BitGravity {
    pub fn get(id: u8) -> Option<BitGravity> {
        match id {
            0 => Some(BitGravity::Forget),
            1 => Some(BitGravity::Static),
            2 => Some(BitGravity::NorthWest),
            3 => Some(BitGravity::North),
            4 => Some(BitGravity::NorthEast),
            5 => Some(BitGravity::West),
            6 => Some(BitGravity::Center),
            7 => Some(BitGravity::East),
            8 => Some(BitGravity::SouthWest),
            9 => Some(BitGravity::South),
            10 => Some(BitGravity::SouthEast),
            _ => None
        }
    }

    pub fn val(&self) -> u32 {
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
impl WindowGravity {
    pub fn get(id: u8) -> Option<WindowGravity> {
        match id {
            0 => Some(WindowGravity::Unmap),
            1 => Some(WindowGravity::Static),
            2 => Some(WindowGravity::NorthWest),
            3 => Some(WindowGravity::North),
            4 => Some(WindowGravity::NorthEast),
            5 => Some(WindowGravity::West),
            6 => Some(WindowGravity::Center),
            7 => Some(WindowGravity::East),
            8 => Some(WindowGravity::SouthWest),
            9 => Some(WindowGravity::South),
            10 => Some(WindowGravity::SouthEast),
            _ => None
        }
    }

    pub fn val(&self) -> u32 {
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
pub enum MapState {
    Unmapped,
    Unviewable,
    Viewable
}
impl MapState {
    pub fn get(id: u8) -> Option<MapState> {
        match id {
            0 => Some(MapState::Unmapped),
            1 => Some(MapState::Unviewable),
            2 => Some(MapState::Viewable),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum FontDrawDirection {
    LeftToRight,
    RightToLeft
}
impl FontDrawDirection {
    pub fn get(id: u8) -> Option<FontDrawDirection> {
        match id {
            0 => Some(FontDrawDirection::LeftToRight),
            1 => Some(FontDrawDirection::RightToLeft),
            _ => None
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
impl GCFunction {
    pub fn val(&self) -> u32 {
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
impl GCLineStyle {
    pub fn val(&self) -> u32 {
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
impl GCCapStyle {
    pub fn val(&self) -> u32 {
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
impl GCJoinStyle {
    pub fn val(&self) -> u32 {
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
impl GCFillStyle {
    pub fn val(&self) -> u32 {
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
impl GCFillRule {
    pub fn val(&self) -> u32 {
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
impl GCSubWindowMode {
    pub fn val(&self) -> u32 {
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
impl GCArcMode {
    pub fn val(&self) -> u32 {
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

    pub fn val(&self) -> u8 {
        match self {
            &MotionNotifyType::Normal => 0,
            &MotionNotifyType::Hint => 1
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

    pub fn val(&self) -> u8 {
        match self {
            &NotifyType::Ancestor => 0,
            &NotifyType::Virtual => 1,
            &NotifyType::Inferior => 2,
            &NotifyType::Nonlinear => 3,
            &NotifyType::NonlinearVirtual => 4
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

    pub fn val(&self) -> u8 {
        match self {
            &FocusType::Ancestor => 0,
            &FocusType::Virtual => 1,
            &FocusType::Inferior => 2,
            &FocusType::Nonlinear => 3,
            &FocusType::NonlinearVirtual => 4,
            &FocusType::Pointer => 5,
            &FocusType::PointerRoot => 6,
            &FocusType::None => 7
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

    pub fn val(&self) -> u8 {
        match self {
            &FocusMode::Normal => 0,
            &FocusMode::Grab => 1,
            &FocusMode::Ungrab => 2,
            &FocusMode::WhileGrabbed => 3
        }
    }
}

#[derive(Debug)]
pub enum InputFocusRevert {
    None,
    PointerRoot,
    Parent
}
impl InputFocusRevert {
    pub fn get(id: u8) -> Option<InputFocusRevert> {
        match id {
            0 => Some(InputFocusRevert::None),
            1 => Some(InputFocusRevert::PointerRoot),
            2 => Some(InputFocusRevert::Parent),
            _ => None
        }
    }

    pub fn val(&self) -> u8 {
        match self {
            &InputFocusRevert::None => 0,
            &InputFocusRevert::PointerRoot => 1,
            &InputFocusRevert::Parent => 2
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

    pub fn val(&self) -> u8 {
        match self {
            &NotifyMode::Normal => 0,
            &NotifyMode::Grab => 1,
            &NotifyMode::Ungrab => 2
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

    pub fn val(&self) -> u8 {
        match self {
            &VisibilityState::Unobscured => 0,
            &VisibilityState::PartiallyObscured => 1,
            &VisibilityState::FullyObscured => 2
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

    pub fn val(&self) -> u8 {
        match self {
            &StackMode::Above => 0,
            &StackMode::Below => 1,
            &StackMode::TopIf => 2,
            &StackMode::BottomIf => 3,
            &StackMode::Opposite => 4
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

    pub fn val(&self) -> u16 {
        match self {
            &ConfigureRequestValues::X => 0x0001,
            &ConfigureRequestValues::Y => 0x0002,
            &ConfigureRequestValues::Width => 0x0004,
            &ConfigureRequestValues::Height => 0x0008,
            &ConfigureRequestValues::BorderWidth => 0x0010,
            &ConfigureRequestValues::Sibling => 0x0020,
            &ConfigureRequestValues::StackMode => 0x0040
        }
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

    pub fn val(&self) -> u8 {
        match self {
            &CirculatePlace::Top => 0,
            &CirculatePlace::Bottom => 1
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

    pub fn val(&self) -> u8 {
        match self {
            &PropertyState::NewValue => 0,
            &PropertyState::Deleted => 1
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

    pub fn val(&self) -> u8 {
        match self {
            &ColormapState::Uninstalled => 0,
            &ColormapState::Installed => 1
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

    pub fn val(&self) -> u8 {
        match self {
            &MappingType::Modifier => 0,
            &MappingType::Keyboard => 1,
            &MappingType::Pointer => 2
        }
    }
}

pub enum SaveSetMode {
    Insert,
    Delete
}
impl SaveSetMode {
    pub fn val(&self) -> u8 {
        match self {
            &SaveSetMode::Insert => 0,
            &SaveSetMode::Delete => 1
        }
    }
}

pub enum CirculateDirection {
    RaiseLowest,
    LowerHighest
}
impl CirculateDirection {
    pub fn val(&self) -> u8 {
        match self {
            &CirculateDirection::RaiseLowest => 0,
            &CirculateDirection::LowerHighest => 1
        }
    }
}

pub enum PropertyChangeMode {
    Replace,
    Prepend,
    Append
}
impl PropertyChangeMode {
    pub fn val(&self) -> u8 {
        match self {
            &PropertyChangeMode::Replace => 0,
            &PropertyChangeMode::Prepend => 1,
            &PropertyChangeMode::Append => 2
        }
    }
}

pub enum PointerMode {
    Synchronous,
    Asynchronous
}
impl PointerMode {
    pub fn val(&self) -> u8 {
        match self {
            &PointerMode::Synchronous => 0,
            &PointerMode::Asynchronous => 1
        }
    }
}

pub enum KeyboardMode {
    Synchronous,
    Asynchronous
}
impl KeyboardMode {
    pub fn val(&self) -> u8 {
        match self {
            &KeyboardMode::Synchronous => 0,
            &KeyboardMode::Asynchronous => 1
        }
    }
}

#[derive(Debug)]
pub enum GraphicsContextMask {
    Function,
    PlaneMask,
    Foreground,
    Background,
    LineWidth,
    LineStyle,
    CapStyle,
    JoinStyle,
    FillStyle,
    FillRule,
    Tile,
    Stipple,
    TileStippleXOrigin,
    TileStippleYOrigin,
    Font,
    SubWindowMode,
    GraphicsExposures,
    ClipXOrigin,
    ClipYOrigin,
    ClipMask,
    DashOffset,
    Dashes,
    ArcMode
}
impl GraphicsContextMask {
    pub fn val(&self) -> u32 {
        match self {
            &GraphicsContextMask::Function => 0x00000001,
            &GraphicsContextMask::PlaneMask => 0x00000002,
            &GraphicsContextMask::Foreground => 0x00000004,
            &GraphicsContextMask::Background => 0x00000008,
            &GraphicsContextMask::LineWidth => 0x00000010,
            &GraphicsContextMask::LineStyle => 0x00000020,
            &GraphicsContextMask::CapStyle => 0x00000040,
            &GraphicsContextMask::JoinStyle => 0x00000080,
            &GraphicsContextMask::FillStyle => 0x00000100,
            &GraphicsContextMask::FillRule => 0x00000200,
            &GraphicsContextMask::Tile => 0x00000400,
            &GraphicsContextMask::Stipple => 0x00000800,
            &GraphicsContextMask::TileStippleXOrigin => 0x00001000,
            &GraphicsContextMask::TileStippleYOrigin => 0x00002000,
            &GraphicsContextMask::Font => 0x00004000,
            &GraphicsContextMask::SubWindowMode => 0x00008000,
            &GraphicsContextMask::GraphicsExposures => 0x00010000,
            &GraphicsContextMask::ClipXOrigin => 0x00020000,
            &GraphicsContextMask::ClipYOrigin => 0x00040000,
            &GraphicsContextMask::ClipMask => 0x00080000,
            &GraphicsContextMask::DashOffset => 0x00100000,
            &GraphicsContextMask::Dashes => 0x00200000,
            &GraphicsContextMask::ArcMode => 0x00400000
        }
    }
}

#[derive(Debug)]
pub enum RectangleOrdering {
    UnSorted,
    YSorted,
    YXSorted,
    YXBanded
}
impl RectangleOrdering {
    pub fn val(&self) -> u8 {
        match self {
            &RectangleOrdering::UnSorted => 0,
            &RectangleOrdering::YSorted => 1,
            &RectangleOrdering::YXSorted => 2,
            &RectangleOrdering::YXBanded => 3
        }
    }
}

#[derive(Debug)]
pub enum CoordinateMode {
    Origin,
    Previous
}
impl CoordinateMode {
    pub fn val(&self) -> u8 {
        match self {
            &CoordinateMode::Origin => 0,
            &CoordinateMode::Previous => 1
        }
    }
}

#[derive(Debug)]
pub enum PolyShape {
    Complex,
    Nonconvex,
    Convex
}
impl PolyShape {
    pub fn val(&self) -> u8 {
        match self {
            &PolyShape::Complex => 0,
            &PolyShape::Nonconvex => 1,
            &PolyShape::Convex => 2,
        }
    }
}

#[derive(Debug)]
pub enum ImageFormat {
    Bitmap,
    XYPixmap,
    ZPixmap
}
impl ImageFormat {
    pub fn val(&self) -> u8 {
        match self {
            &ImageFormat::Bitmap => 0,
            &ImageFormat::XYPixmap => 1,
            &ImageFormat::ZPixmap => 2,
        }
    }
}

#[derive(Debug)]
pub enum AllocMode {
    None,
    All
}
impl AllocMode {
    pub fn val(&self) -> u8 {
        match self {
            &AllocMode::None => 0,
            &AllocMode::All => 1
        }
    }
}

#[derive(Debug)]
pub enum SizeClass {
    Cursor,
    Tile,
    Stipple
}
impl SizeClass {
    pub fn val(&self) -> u8 {
        match self {
            &SizeClass::Cursor => 0,
            &SizeClass::Tile => 1,
            &SizeClass::Stipple => 2
        }
    }
}

#[derive(Debug)]
pub enum KeyboardControlLedMode {
    Off,
    On
}
impl KeyboardControlLedMode {
    pub fn val(&self) -> u8 {
        match self {
            &KeyboardControlLedMode::Off => 0,
            &KeyboardControlLedMode::On => 1
        }
    }
}

#[derive(Debug)]
pub enum KeyboardControlAutoRepeatMode {
    Off,
    On,
    Default
}
impl KeyboardControlAutoRepeatMode {
    pub fn val(&self) -> u8 {
        match self {
            &KeyboardControlAutoRepeatMode::Off => 0,
            &KeyboardControlAutoRepeatMode::On => 1,
            &KeyboardControlAutoRepeatMode::Default => 2
        }
    }

    pub fn get(id: u8) -> Option<KeyboardControlAutoRepeatMode> {
        match id {
            0 => Some(KeyboardControlAutoRepeatMode::Off),
            1 => Some(KeyboardControlAutoRepeatMode::On),
            2 => Some(KeyboardControlAutoRepeatMode::Default),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum YesNoDefault {
    No,
    Yes,
    Default
}
impl YesNoDefault {
    pub fn val(&self) -> u8 {
        match self {
            &YesNoDefault::No => 0,
            &YesNoDefault::Yes => 1,
            &YesNoDefault::Default => 2
        }
    }
}

#[derive(Debug)]
pub enum HostFamily {
    Internet,
    DECnet,
    Chaos,
    ServerInterpreted,
    InternetV6
}
impl HostFamily {
    pub fn val(&self) -> u8 {
        match self {
            &HostFamily::Internet => 0,
            &HostFamily::DECnet => 1,
            &HostFamily::Chaos => 2,
            &HostFamily::ServerInterpreted => 5,
            &HostFamily::InternetV6 => 6,
        }
    }

    pub fn get(id: u8) -> Option<HostFamily> {
        match id {
            0 => Some(HostFamily::Internet),
            1 => Some(HostFamily::DECnet),
            2 => Some(HostFamily::Chaos),
            5 => Some(HostFamily::ServerInterpreted),
            6 => Some(HostFamily::InternetV6),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum ChangeHostMode {
    Insert,
    Delete
}
impl ChangeHostMode {
    pub fn val(&self) -> u8 {
        match self {
            &ChangeHostMode::Insert => 0,
            &ChangeHostMode::Delete => 1
        }
    }
}

#[derive(Debug)]
pub enum CloseDownMode {
    Destroy,
    RetainPermanent,
    RetainTemporary
}
impl CloseDownMode {
    pub fn val(&self) -> u8 {
        match self {
            &CloseDownMode::Destroy => 0,
            &CloseDownMode::RetainPermanent => 1,
            &CloseDownMode::RetainTemporary => 2
        }
    }
}

#[derive(Debug)]
pub enum GrabStatus {
    Success,
    AlreadyGrabbed,
    InvalidTime,
    NotViewable,
    Frozen
}
impl GrabStatus {
    pub fn get(id: u8) -> Option<GrabStatus> {
        match id {
            0 => Some(GrabStatus::Success),
            1 => Some(GrabStatus::AlreadyGrabbed),
            2 => Some(GrabStatus::InvalidTime),
            3 => Some(GrabStatus::NotViewable),
            4 => Some(GrabStatus::Frozen),
            _ => None
        }
    }
}

#[derive(Debug)]
pub enum SetModifierMappingStatus {
    Success,
    Busy,
    Failed
}
impl SetModifierMappingStatus {
    pub fn get(id: u8) -> Option<SetModifierMappingStatus> {
        match id {
            0 => Some(SetModifierMappingStatus::Success),
            1 => Some(SetModifierMappingStatus::Busy),
            2 => Some(SetModifierMappingStatus::Failed),
            _ => None
        }
    }
}

//
//
//
//
////////////////////////////////////////
/// VALUES
////////////////////////////////////////
//
//
//
//

#[derive(Debug)]
pub enum WindowValue {
    BackgroundPixmap(u32),
    BackgroundPixel(u32),
    BorderPixmap(u32),
    BorderPixel(u32),
    BitGravity(BitGravity),
    WinGravity(WindowGravity),
    BackingStore(WindowBackingStore),
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

#[derive(Debug)]
pub enum KeyboardControlValue {
    KeyClickPercent(u8),
    BellPercent(u8),
    BellPitch(i16),
    BellDuration(i16),
    Led(u8),
    LedMode(KeyboardControlLedMode),
    Key(char),
    AutoRepeatMode(KeyboardControlAutoRepeatMode)
}

//
//
//
//
////////////////////////////////////////
/// VALUES METHODS
////////////////////////////////////////
//
//
//
//

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

    fn write(&self, client: &mut XClient) {
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

    fn write(&self, client: &mut XClient) {
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

impl Value for KeyboardControlValue {
    fn get_mask(&self) -> u32 {
        match self {
            &KeyboardControlValue::KeyClickPercent(_) => 0x0001,
            &KeyboardControlValue::BellPercent(_) => 0x0002,
            &KeyboardControlValue::BellPitch(_) =>  0x0004,
            &KeyboardControlValue::BellDuration(_) => 0x0008,
            &KeyboardControlValue::Led(_) =>  0x0010,
            &KeyboardControlValue::LedMode(_) => 0x0020,
            &KeyboardControlValue::Key(_) => 0x0040,
            &KeyboardControlValue::AutoRepeatMode(_) => 0x0040
        }
    }

    fn write(&self, client: &mut XClient) {
        match self {
            &KeyboardControlValue::KeyClickPercent(val) => client.write_u8(val),
            &KeyboardControlValue::BellPercent(val) => client.write_u8(val),
            &KeyboardControlValue::BellPitch(val) => client.write_i16(val),
            &KeyboardControlValue::BellDuration(val) => client.write_i16(val),
            &KeyboardControlValue::Led(val) => client.write_u8(val),
            &KeyboardControlValue::LedMode(ref val) => client.write_u8(val.val()),
            &KeyboardControlValue::Key(val) => client.write_char(val),
            &KeyboardControlValue::AutoRepeatMode(ref val) => client.write_u8(val.val())
        };
    }
}
