//! Minimal JS bindings for the torii client.

use std::str::FromStr;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use torii_grpc::types::{Clause, KeysClause, Query};
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use serde_json::Value;
use torii_grpc::types::schema::Entity;

pub fn parse_entities_as_json_str(entities: Vec<Entity>) -> Value {
    entities
        .into_iter()
        .map(|entity| {
            let entity_key = format!("{:#x}", entity.hashed_keys);
            let models_map = entity
                .models
                .into_iter()
                .map(|model| {
                    let model_map = model
                        .members
                        .iter()
                        .map(|member| (member.name.to_owned(), parse_ty_as_json_str(&member.ty)))
                        .collect::<serde_json::Map<String, Value>>();

                    (model.name, model_map.into())
                })
                .collect::<serde_json::Map<String, Value>>();

            (entity_key, models_map.into())
        })
        .collect::<serde_json::Map<String, Value>>()
        .into()
}

pub fn parse_ty_as_json_str(ty: &Ty) -> Value {
    match ty {
        Ty::Primitive(primitive) => serde_json::json!({
            "type": primitive.to_string(),
            "value": primitive_value_json(*primitive)
        }),

        Ty::Struct(struct_ty) => serde_json::json!({
            "type": "struct",
            "value": struct_ty
            .children
            .iter()
            .map(|child| (child.name.to_owned(), parse_ty_as_json_str(&child.ty)))
            .collect::<serde_json::Map<String, Value>>()
        }),

        Ty::Enum(enum_ty) => serde_json::json!({
            "type": "enum",
            "value": if let Some(option) = enum_ty.option {
                option.into()
            } else {
                Value::Null
            }
        }),

        Ty::Tuple(_) => unimplemented!("tuple not supported"),
    }
}

fn primitive_value_json(primitive: Primitive) -> Value {
    match primitive {
        Primitive::Bool(Some(value)) => Value::Bool(value),
        Primitive::U8(Some(value)) => Value::Number(value.into()),
        Primitive::U16(Some(value)) => Value::Number(value.into()),
        Primitive::U32(Some(value)) => Value::Number(value.into()),
        Primitive::U64(Some(value)) => Value::Number(value.into()),
        Primitive::USize(Some(value)) => Value::Number(value.into()),
        Primitive::U128(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::U256(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::Felt252(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::ClassHash(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::ContractAddress(Some(value)) => Value::String(format!("{value:#x}")),
        _ => Value::Null,
    }
}

type JsFieldElement = JsValue;

// TODO: remove this in favour of the new EntityQuery
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EntityModel {
    pub model: String,
    pub keys: Vec<FieldElement>,
}

impl From<EntityModel> for KeysClause {
    fn from(value: EntityModel) -> Self {
        Self {
            model: value.model,
            keys: value.keys,
        }
    }
}

#[wasm_bindgen(typescript_custom_section)]
pub const ENTITY_MODEL_STR: &'static str = r#"
export interface EntityModel {
    model: string;
    keys: string[];
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "EntityModel")]
    pub type IEntityModel;
}

impl TryFrom<IEntityModel> for KeysClause {
    type Error = serde_wasm_bindgen::Error;
    fn try_from(value: IEntityModel) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value::<EntityModel>(value.into()).map(|e| e.into())
    }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientConfig {
    #[serde(rename = "rpcUrl")]
    pub rpc_url: String,
    #[serde(rename = "toriiUrl")]
    pub torii_url: String,
    #[serde(rename = "worldAddress")]
    pub world_address: String,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[wasm_bindgen]
pub struct Client {
    inner: torii_client::client::Client,
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(js_name = getEntities)]
    pub async fn get_entities(&self, limit: u32, offset: u32) -> Result<JsValue, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let results = self
            .inner
            .entities(Query {
                clause: None,
                limit,
                offset,
            })
            .await;

        match results {
            Ok(entities) => Ok(js_sys::JSON::parse(
                &parse_entities_as_json_str(entities).to_string(),
            )?),
            Err(err) => Err(JsValue::from(format!("failed to get entities: {err}"))),
        }
    }

    #[wasm_bindgen(js_name = getEntitiesByKeys)]
    pub async fn get_entities_by_keys(
        &self,
        model: &str,
        keys: Vec<JsFieldElement>,
        limit: u32,
        offset: u32,
    ) -> Result<JsValue, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let keys = keys
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<FieldElement>)
            .collect::<Result<Vec<FieldElement>, _>>()
            .map_err(|err| JsValue::from(format!("failed to parse entity keys: {err}")))?;

        let results = self
            .inner
            .entities(Query {
                clause: Some(Clause::Keys(KeysClause {
                    model: model.to_string(),
                    keys,
                })),
                limit,
                offset,
            })
            .await;

        match results {
            Ok(entities) => Ok(js_sys::JSON::parse(
                &parse_entities_as_json_str(entities).to_string(),
            )?),
            Err(err) => Err(JsValue::from(format!("failed to get entities: {err}"))),
        }
    }

    /// Retrieves the model value of an entity. Will fetch from remote if the requested entity is not one of the entities that are being synced.
    #[wasm_bindgen(js_name = getModelValue)]
    pub async fn get_model_value(
        &self,
        model: &str,
        keys: Vec<JsFieldElement>,
    ) -> Result<JsValue, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let keys = keys
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<FieldElement>)
            .collect::<Result<Vec<FieldElement>, _>>()
            .map_err(|err| JsValue::from(format!("failed to parse entity keys: {err}")))?;

        match self
            .inner
            .model(&KeysClause {
                model: model.to_string(),
                keys,
            })
            .await
        {
            Ok(Some(ty)) => Ok(js_sys::JSON::parse(&parse_ty_as_json_str(&ty).to_string())?),
            Ok(None) => Ok(JsValue::NULL),

            Err(err) => Err(JsValue::from(format!("failed to get entity: {err}"))),
        }
    }

    /// Register new entities to be synced.
    #[wasm_bindgen(js_name = addModelsToSync)]
    pub async unsafe fn add_models_to_sync(
        &self,
        models: Vec<IEntityModel>,
    ) -> Result<(), JsValue> {
        log("adding models to sync...");

        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let models = models
            .into_iter()
            .map(|e| TryInto::<KeysClause>::try_into(e))
            .collect::<Result<Vec<_>, _>>()?;

        self.inner
            .add_models_to_sync(models)
            .await
            .map_err(|err| JsValue::from(err.to_string()))
    }

    /// Remove the entities from being synced.
    #[wasm_bindgen(js_name = removeModelsToSync)]
    pub async unsafe fn remove_models_to_sync(
        &self,
        models: Vec<IEntityModel>,
    ) -> Result<(), JsValue> {
        log("removing models to sync...");

        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let models = models
            .into_iter()
            .map(|e| TryInto::<KeysClause>::try_into(e))
            .collect::<Result<Vec<_>, _>>()?;

        self.inner
            .remove_models_to_sync(models)
            .await
            .map_err(|err| JsValue::from(err.to_string()))
    }

    /// Register a callback to be called every time the specified synced entity's value changes.
    #[wasm_bindgen(js_name = onSyncModelChange)]
    pub fn on_sync_model_change(
        &self,
        model: IEntityModel,
        callback: js_sys::Function,
    ) -> Result<(), JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let model = serde_wasm_bindgen::from_value::<EntityModel>(model.into())?;
        let name = cairo_short_string_to_felt(&model.model).expect("invalid model name");
        let mut rcv = self
            .inner
            .storage()
            .add_listener(name, &model.keys)
            .unwrap();

        wasm_bindgen_futures::spawn_local(async move {
            while rcv.next().await.is_some() {
                let _ = callback.call0(&JsValue::null());
            }
        });

        Ok(())
    }

    #[wasm_bindgen(js_name = onEntityUpdated)]
    pub async fn on_entity_updated(
        &self,
        ids: Option<Vec<String>>,
        callback: js_sys::Function,
    ) -> Result<(), JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let ids = ids
            .unwrap_or(vec![])
            .into_iter()
            .map(|id| {
                FieldElement::from_str(&id)
                    .map_err(|err| JsValue::from(format!("failed to parse entity id: {err}")))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut stream = self.inner.on_entity_updated(ids).await.unwrap();

        wasm_bindgen_futures::spawn_local(async move {
            while let Some(update) = stream.next().await {
                let entity = update.expect("no updated entity");
                let json_str = parse_entities_as_json_str(vec![entity]).to_string();
                let _ = callback.call1(
                    &JsValue::null(),
                    &js_sys::JSON::parse(&json_str).expect("json parse failed"),
                );
            }
        });

        Ok(())
    }
}

/// Create the a client with the given configurations.
#[wasm_bindgen(js_name = createClient)]
#[allow(non_snake_case)]
pub async fn create_client(
    initialModelsToSync: Vec<IEntityModel>,
    config: ClientConfig,
) -> Result<Client, JsValue> {
    #[cfg(feature = "console-error-panic")]
    console_error_panic_hook::set_once();

    let ClientConfig {
        rpc_url,
        torii_url,
        world_address,
    } = config;

    let models = initialModelsToSync
        .into_iter()
        .map(|e| TryInto::<KeysClause>::try_into(e))
        .collect::<Result<Vec<_>, _>>()?;

    let world_address = FieldElement::from_str(&world_address)
        .map_err(|err| JsValue::from(format!("failed to parse world address: {err}")))?;

    let client = torii_client::client::Client::new(torii_url, rpc_url, world_address, Some(models))
        .await
        .map_err(|err| JsValue::from(format!("failed to build client: {err}")))?;

    wasm_bindgen_futures::spawn_local(client.start_subscription().await.map_err(|err| {
        JsValue::from(format!(
            "failed to start torii client subscription service: {err}"
        ))
    })?);

    Ok(Client { inner: client })
}
