# X Rust Bindings, aka XRB
Pure Rust bindings for X11 protocol.

# READ ME
This is still a work in progress.
If you use this, please **let me know what you want done first** and **let me know what doesn't work**.
Both can be done by submitting an issue.
There is a lot of ground to cover so progress may seem a bit slow, but if there are a few methods you want bindings for, or you find some bugs, they will be put at the top of my list.

# So what's done?
- Connect with no auth
- All standard requests (not all have been tested)
- Subscribe to events
- Get events, errors, and two replies from the X Server
    - Temporarily ignore messages from the X Server until you get an error/reply with your sequence number (stores messages for later usage)

# How Does It Work?
- A listener thread is spawned that reads messages from the server forever
    - This prevents deadlocking, since (by the spec) the X Server MAY not accept a new message until it has sent the reply to previous one
- All write operations are done on the main thread

# Usage
See tests/main.rs for some example usage.
1. Do initial setup (create windows, subscribe to events, etc)
2. Run an event loop using client.wait_for_message()
    - Responds with replies, errors, and events

# TODO
    - Replies (Crl+F "▶")
        - NOTE: Even though I may "complete" all requests when done the above point, the replies may not be done. Check!
    - Map functions to objects (ie `window.destroy()` instead of `client.destroy_window(window.wid)`)
    - Async versions for functions with replies (ie query_font(...), font.query(...))
        - So they don't have to manually call wait_for_response(seq)
    - Some skipped stuff
        - read_keymap_notify
        - set_font_path
        - change_keyboard_mapping
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
