module 0x8fd379246834eac74b8419ffda202cf8051f7a03::hello_strings {
    use 0x1::string::{Self, String};

    /// Resource that wraps a string
    struct Msg has key { value: String }

    public entry fun main(name: String) {
        let greeting = string::utf8(b"Hello, ");
        string::append(&mut greeting, name);
    }

    /// Create Msg resource on an account
    public entry fun publish(account: &signer, msg: String) {
        move_to(account, Msg { value: msg })
    }

    /// Change an existing Msg resource
    public entry fun update(addr: address, new_msg: String) acquires Msg {
        let msg_ref = &mut borrow_global_mut<Msg>(addr).value;
        *msg_ref = new_msg;
    }
}
