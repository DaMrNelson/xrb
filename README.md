# X Rust Bindings, aka XRB
Pure Rust bindings for X11 protocol.

# WIP NOTICE
This is still a work in progress, and things may not work. Feel free to submit suggestions, issues, or any other types of feedback though!

# So what's done?
- Connect with no auth
- Create and map a window with a pixmap and graphics context
- Subscribe to events
- Get events, errors, and two replies from the X Server
    - Temporarily ignore messages from the X Server until you get an error/reply with your sequence number (stores messages for later usage)

# How Does It Work?
- Connection is all async
- A listener thread is spawned that reads messages from the server forever
    - This prevents deadlocking, since (by the spec) the X Server MAY not accept a new message until it has sent the reply to previous one
- All write operations are done on the main thread

# Usage
See tests/main.rs for some example usage.
1. Do initial setup (create windows, subscribe to events, etc)
2. Run an event loop using client.wait_for_message()
    - Responds with replies, errors, and events

# TODO
    - All request types
        - Continue at GrabPointer
    - Replies (Crl+F "▶")
        - Some are implemented. Lots are not though.
    - Map functions to objects (ie `window.destroy()` instead of `client.destroy_window(window.wid)`)
    - read_keymap_notify
    - send_event (just procrastinating it rn)
    - Multithread usage?
        - Thread lock when creating new resource IDs. Or maybe just thread lock the entire thing? Idk yet.
    - Allow re-use of used resource IDs
    - Don't unwrap and panic everywhere
    - Write some examples
    - Write some docs
        - Write manual docs for the important stuff
            - Search for things like "[len] [type] [name]\n[special index] [special value]
                - Ie SelectionNotify's time and property. You can specify it, or you can leave it blank
                - You can tell the difference because enums are "[len] [BLANK] [name]" while these are "[len] [type] [name]"
        - Use autodocs for the rest. The poor saps can rely on examples and intuition for a bit
