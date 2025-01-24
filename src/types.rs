use presage::libsignal_service::configuration::SignalServers;
use presage::libsignal_service::prelude::phonenumber::PhoneNumber;
use presage::libsignal_service::prelude::ProfileKey;
use presage::libsignal_service::prelude::Uuid;
use presage::libsignal_service::zkgroup::GroupMasterKeyBytes;
use std::path::PathBuf;
use url::Url;

pub enum Recipient {
    Contact(Uuid),
    Group(GroupMasterKeyBytes),
}

pub struct Args {
    pub db_path: Option<PathBuf>,
    pub passphrase: Option<String>,
    pub subcommand: Cmd,
}
impl Args {
    pub fn default() -> Args {
        Args {
            db_path: None,
            passphrase: None,
            subcommand: Cmd::Receive,
            // subcommand: Cmd::Send { uuid: uuid::uuid!(""), message: String::from("todo!()"), attachment_filepath: vec![] },
            // subcommand: Cmd::Whoami,
            // subcommand: Cmd::SyncContacts,
        }
    }
}

pub enum Cmd {
    Register {
        servers: SignalServers,
        phone_number: PhoneNumber,
        use_voice_call: bool,
        captcha: Url,
        force: bool,
    },
    LinkDevice {
        /// Possible values: staging, production
        servers: SignalServers,
        device_name: String,
    },
    AddDevice {
        url: Url,
    },
    UnlinkDevice {
        device_id: i64,
    },
    ListDevices,
    Whoami,
    RetrieveProfile {
        /// Id of the user to retrieve the profile. When omitted, retrieves the registered user
        /// profile.
        uuid: Uuid,
        /// Base64-encoded profile key of user to be able to access their profile
        profile_key: Option<ProfileKey>,
    },
    Receive,
    ListGroups,
    ListContacts,
    ListMessages {
        recipient_uuid: Option<Uuid>,
        group_master_key: Option<GroupMasterKeyBytes>,
        from: Option<u64>,
    },
    ListStickerPacks,
    GetContact {
        uuid: Uuid,
    },
    FindContact {
        uuid: Option<Uuid>,
        phone_number: Option<PhoneNumber>,
        name: Option<String>,
    },
    Send {
        uuid: Uuid,
        message: String,
        attachment_filepath: Vec<PathBuf>,
    },
    SendToGroup {
        message: String,
        master_key: GroupMasterKeyBytes,
        attachment_filepath: Vec<PathBuf>,
    },
    SyncContacts,
    Stats,
}
