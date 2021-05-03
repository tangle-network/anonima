// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::MethodNum;
use crate::Serialized;
use super::Message;
use address::Address;
use derive_builder::Builder;
use encoding::Cbor;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Default Unsigned VM message type which includes all data needed for a state transition
///
/// Usage:
/// ```
/// use forest_message::{UnsignedMessage, Message};
/// use vm::{TokenAmount, Serialized, MethodNum};
/// use address::Address;
///
/// // Use the builder pattern to generate a message
/// let message = UnsignedMessage::builder()
///     .to(Address::new_id(0))
///     .from(Address::new_id(1))
///     .sequence(0) // optional
///     .value(TokenAmount::from(0u8)) // optional
///     .method_num(MethodNum::default()) // optional
///     .params(Serialized::default()) // optional
///     .gas_limit(0) // optional
///     .version(0) // optional
///     .build()
///     .unwrap();
///
// /// Commands can be chained, or built separately
/// let mut message_builder = UnsignedMessage::builder();
/// message_builder.sequence(1);
/// message_builder.from(Address::new_id(0));
/// message_builder.to(Address::new_id(1));
/// let msg = message_builder.build().unwrap();
/// assert_eq!(msg.sequence(), 1);
/// ```
#[derive(PartialEq, Clone, Debug, Builder, Hash, Eq)]
#[builder(name = "MessageBuilder")]
pub struct UnsignedMessage {
    #[builder(default)]
    pub version: i64,
    pub from: Address,
    pub to: Address,
    #[builder(default)]
    pub method_num: MethodNum,
    #[builder(default)]
    pub params: Serialized,
}

impl UnsignedMessage {
    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }

    /// Helper function to convert the message into signing bytes.
    /// This function returns the message `Cid` bytes.
    pub fn to_signing_bytes(&self) -> Vec<u8> {
        // Safe to unwrap here, unsigned message cannot fail to serialize.
        // self.cid().unwrap().to_bytes()
        // 
        // TODO: Add proper impl for anonima
        return vec![];
    }
}

impl Serialize for UnsignedMessage {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (
            &self.version,
            &self.to,
            &self.from,
            &self.method_num,
            &self.params,
        )
            .serialize(s)
    }
}

impl<'de> Deserialize<'de> for UnsignedMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (
            version,
            to,
            from,
            method_num,
            params,
        ) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            version,
            from,
            to,
            method_num,
            params,
        })
    }
}

impl Message for UnsignedMessage {
    fn from(&self) -> &Address {
        &self.from
    }
    fn to(&self) -> &Address {
        &self.to
    }
    fn method_num(&self) -> MethodNum {
        self.method_num
    }
    fn params(&self) -> &Serialized {
        &self.params
    }
}

impl Cbor for UnsignedMessage {}

#[cfg(feature = "json")]
pub mod json {
    use super::*;
    use address::json::AddressJson;
    use cid::Cid;
    use num_bigint::bigint_ser;
    use serde::{de, ser};

    /// Wrapper for serializing and deserializing a UnsignedMessage from JSON.
    #[derive(Deserialize, Serialize, Debug)]
    #[serde(transparent)]
    pub struct UnsignedMessageJson(#[serde(with = "self")] pub UnsignedMessage);

    /// Wrapper for serializing a UnsignedMessage reference to JSON.
    #[derive(Serialize)]
    #[serde(transparent)]
    pub struct UnsignedMessageJsonRef<'a>(#[serde(with = "self")] pub &'a UnsignedMessage);

    impl From<UnsignedMessageJson> for UnsignedMessage {
        fn from(wrapper: UnsignedMessageJson) -> Self {
            wrapper.0
        }
    }

    impl From<UnsignedMessage> for UnsignedMessageJson {
        fn from(wrapper: UnsignedMessage) -> Self {
            UnsignedMessageJson(wrapper)
        }
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct JsonHelper {
        version: i64,
        to: AddressJson,
        from: AddressJson,
        method_num: u64,
        params: Option<String>,
        #[serde(default, rename = "CID", with = "cid::json::opt")]
        cid: Option<Cid>,
    }

    pub fn serialize<S>(m: &UnsignedMessage, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        JsonHelper {
            version: m.version,
            to: m.to.into(),
            from: m.from.into(),
            method_num: m.method_num,
            params: Some(base64::encode(m.params.bytes())),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<UnsignedMessage, D::Error>
    where
        D: Deserializer<'de>,
    {
        let m: JsonHelper = Deserialize::deserialize(deserializer)?;
        Ok(UnsignedMessage {
            version: m.version,
            to: m.to.into(),
            from: m.from.into(),
            method_num: m.method_num,
            params: Serialized::new(
                base64::decode(&m.params.unwrap_or_else(|| "".to_string()))
                    .map_err(de::Error::custom)?,
            ),
        })
    }

    pub mod vec {
        use super::*;
        use forest_json_utils::GoVecVisitor;
        use serde::ser::SerializeSeq;

        pub fn serialize<S>(m: &[UnsignedMessage], serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut seq = serializer.serialize_seq(Some(m.len()))?;
            for e in m {
                seq.serialize_element(&UnsignedMessageJsonRef(e))?;
            }
            seq.end()
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<UnsignedMessage>, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer
                .deserialize_any(GoVecVisitor::<UnsignedMessage, UnsignedMessageJson>::new())
        }
    }
}
