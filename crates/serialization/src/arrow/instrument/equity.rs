// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2026 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! Arrow serialization for Equity instruments.

use std::{borrow::Borrow, collections::HashMap, str::FromStr, sync::Arc};

use arrow::{
    array::{Array, StringArray, StringBuilder, UInt8Array, UInt64Array},
    datatypes::{DataType, Field, Schema},
    error::ArrowError,
    record_batch::RecordBatch,
};
use nautilus_core::Params;
use nautilus_model::{
    identifiers::{InstrumentId, Symbol},
    instruments::equity::Equity,
    types::{price::Price, quantity::Quantity},
};
use rust_decimal::Decimal;
use ustr::Ustr;

use crate::arrow::{
    ArrowSchemaProvider, EncodeToRecordBatch, EncodingError, KEY_INSTRUMENT_ID,
    KEY_PRICE_PRECISION, extract_column, extract_column_by_name,
    extract_optional_string_column_by_name, json_string_field, metadata_with_type_name,
    optional_ustr_value, record_batch_with_timestamps, record_batch_with_u64_timestamps,
    timestamp_data_type,
};

impl ArrowSchemaProvider for Equity {
    fn get_schema(metadata: Option<HashMap<String, String>>) -> Schema {
        let fields = vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("raw_symbol", DataType::Utf8, false),
            Field::new("currency", DataType::Utf8, false),
            Field::new("price_precision", DataType::UInt8, false),
            Field::new("price_increment", DataType::Utf8, false),
            Field::new("lot_size", DataType::Utf8, true), // nullable
            Field::new("isin", DataType::Utf8, true),     // nullable
            Field::new("max_quantity", DataType::Utf8, true), // nullable
            Field::new("min_quantity", DataType::Utf8, true), // nullable
            Field::new("max_price", DataType::Utf8, true), // nullable
            Field::new("min_price", DataType::Utf8, true), // nullable
            Field::new("margin_init", DataType::Utf8, false),
            Field::new("margin_maint", DataType::Utf8, false),
            Field::new("tick_scheme", DataType::Utf8, true),
            json_string_field("info", true),
            Field::new("ts_event", timestamp_data_type(), false),
            Field::new("ts_init", timestamp_data_type(), false),
            // Appended and nullable so batches written without them still decode (as 0 / "1").
            Field::new("size_precision", DataType::UInt8, true),
            Field::new("size_increment", DataType::Utf8, true),
        ];

        Schema::new_with_metadata(fields, metadata_with_type_name("Equity", metadata))
    }
}

impl EncodeToRecordBatch for Equity {
    fn encode_batch<T>(
        #[allow(unused)] metadata: &HashMap<String, String>,
        data: &[T],
    ) -> Result<RecordBatch, ArrowError>
    where
        T: std::borrow::Borrow<Self>,
    {
        let mut id_builder = StringBuilder::new();
        let mut raw_symbol_builder = StringBuilder::new();
        let mut currency_builder = StringBuilder::new();
        let mut price_precision_builder = UInt8Array::builder(data.len());
        let mut price_increment_builder = StringBuilder::new();
        let mut lot_size_builder = StringBuilder::new();
        let mut isin_builder = StringBuilder::new();
        let mut max_quantity_builder = StringBuilder::new();
        let mut min_quantity_builder = StringBuilder::new();
        let mut max_price_builder = StringBuilder::new();
        let mut min_price_builder = StringBuilder::new();
        let mut margin_init_builder = StringBuilder::new();
        let mut margin_maint_builder = StringBuilder::new();
        let mut tick_scheme_builder = StringBuilder::new();
        let mut info_builder = StringBuilder::new();
        let mut ts_event_builder = UInt64Array::builder(data.len());
        let mut ts_init_builder = UInt64Array::builder(data.len());
        let mut size_precision_builder = UInt8Array::builder(data.len());
        let mut size_increment_builder = StringBuilder::new();

        for equity in data.iter().map(Borrow::borrow) {
            id_builder.append_value(equity.id.to_string());
            raw_symbol_builder.append_value(equity.raw_symbol);
            currency_builder.append_value(equity.currency.to_string());
            price_precision_builder.append_value(equity.price_precision);
            price_increment_builder.append_value(equity.price_increment.to_string());

            if let Some(lot_size) = equity.lot_size {
                lot_size_builder.append_value(lot_size.to_string());
            } else {
                lot_size_builder.append_null();
            }

            if let Some(isin) = equity.isin {
                isin_builder.append_value(isin);
            } else {
                isin_builder.append_null();
            }

            if let Some(max_qty) = equity.max_quantity {
                max_quantity_builder.append_value(max_qty.to_string());
            } else {
                max_quantity_builder.append_null();
            }

            if let Some(min_qty) = equity.min_quantity {
                min_quantity_builder.append_value(min_qty.to_string());
            } else {
                min_quantity_builder.append_null();
            }

            if let Some(max_p) = equity.max_price {
                max_price_builder.append_value(max_p.to_string());
            } else {
                max_price_builder.append_null();
            }

            if let Some(min_p) = equity.min_price {
                min_price_builder.append_value(min_p.to_string());
            } else {
                min_price_builder.append_null();
            }

            margin_init_builder.append_value(equity.margin_init.to_string());
            margin_maint_builder.append_value(equity.margin_maint.to_string());

            if let Some(tick_scheme) = equity.tick_scheme {
                tick_scheme_builder.append_value(tick_scheme);
            } else {
                tick_scheme_builder.append_null();
            }

            if let Some(ref info) = equity.info {
                match serde_json::to_string(info) {
                    Ok(json) => {
                        info_builder.append_value(json);
                    }
                    Err(e) => {
                        return Err(ArrowError::InvalidArgumentError(format!(
                            "Failed to serialize info dict to JSON: {e}"
                        )));
                    }
                }
            } else {
                info_builder.append_null();
            }

            ts_event_builder.append_value(equity.ts_event.as_u64());
            ts_init_builder.append_value(equity.ts_init.as_u64());
            size_precision_builder.append_value(equity.size_precision);
            size_increment_builder.append_value(equity.size_increment.to_string());
        }

        record_batch_with_timestamps(
            Self::get_schema(Some(metadata.clone())).into(),
            vec![
                Arc::new(id_builder.finish()),
                Arc::new(raw_symbol_builder.finish()),
                Arc::new(currency_builder.finish()),
                Arc::new(price_precision_builder.finish()),
                Arc::new(price_increment_builder.finish()),
                Arc::new(lot_size_builder.finish()),
                Arc::new(isin_builder.finish()),
                Arc::new(max_quantity_builder.finish()),
                Arc::new(min_quantity_builder.finish()),
                Arc::new(max_price_builder.finish()),
                Arc::new(min_price_builder.finish()),
                Arc::new(margin_init_builder.finish()),
                Arc::new(margin_maint_builder.finish()),
                Arc::new(tick_scheme_builder.finish()),
                Arc::new(info_builder.finish()),
                Arc::new(ts_event_builder.finish()),
                Arc::new(ts_init_builder.finish()),
                Arc::new(size_precision_builder.finish()),
                Arc::new(size_increment_builder.finish()),
            ],
        )
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert(KEY_INSTRUMENT_ID.to_string(), self.id.to_string());
        metadata.insert(
            KEY_PRICE_PRECISION.to_string(),
            self.price_precision.to_string(),
        );
        metadata
    }
}

/// Decodes [`Equity`] instruments from a record batch.
///
/// Not a [`DecodeFromRecordBatch`] implementation because that trait requires `Into<Data>`.
///
/// # Errors
///
/// Returns an `EncodingError` if the record batch cannot be decoded.
///
/// [`DecodeFromRecordBatch`]: crate::arrow::DecodeFromRecordBatch
pub fn decode_equity_batch(
    #[allow(unused)] metadata: &HashMap<String, String>,
    record_batch: &RecordBatch,
) -> Result<Vec<Equity>, EncodingError> {
    let record_batch = record_batch_with_u64_timestamps(record_batch)?;
    let record_batch = &record_batch;
    let cols = record_batch.columns();
    let num_rows = record_batch.num_rows();

    // Read precision from data columns (it's in the schema)
    let id_values = extract_column::<StringArray>(cols, "id", 0, DataType::Utf8)?;
    let raw_symbol_values = extract_column::<StringArray>(cols, "raw_symbol", 1, DataType::Utf8)?;
    let currency_values = extract_column::<StringArray>(cols, "currency", 2, DataType::Utf8)?;
    let price_precision_values =
        extract_column::<UInt8Array>(cols, "price_precision", 3, DataType::UInt8)?;
    let price_increment_values =
        extract_column::<StringArray>(cols, "price_increment", 4, DataType::Utf8)?;
    let lot_size_values = cols
        .get(5)
        .ok_or_else(|| EncodingError::MissingColumn("lot_size", 5))?;
    let isin_values = cols
        .get(6)
        .ok_or_else(|| EncodingError::MissingColumn("isin", 6))?;
    let max_quantity_values = cols
        .get(7)
        .ok_or_else(|| EncodingError::MissingColumn("max_quantity", 7))?;
    let min_quantity_values = cols
        .get(8)
        .ok_or_else(|| EncodingError::MissingColumn("min_quantity", 8))?;
    let max_price_values = cols
        .get(9)
        .ok_or_else(|| EncodingError::MissingColumn("max_price", 9))?;
    let min_price_values = cols
        .get(10)
        .ok_or_else(|| EncodingError::MissingColumn("min_price", 10))?;
    let margin_init_values =
        extract_column::<StringArray>(cols, "margin_init", 11, DataType::Utf8)?;
    let margin_maint_values =
        extract_column::<StringArray>(cols, "margin_maint", 12, DataType::Utf8)?;
    let tick_scheme_values = extract_optional_string_column_by_name(record_batch, "tick_scheme")?;
    let info_values = extract_column_by_name::<StringArray>(record_batch, "info", DataType::Utf8)?;
    let ts_event_values =
        extract_column_by_name::<UInt64Array>(record_batch, "ts_event", DataType::UInt64)?;
    let ts_init_values =
        extract_column_by_name::<UInt64Array>(record_batch, "ts_init", DataType::UInt64)?;
    // Optional: absent in batches written before these columns existed.
    let size_precision_values = match record_batch.schema().index_of("size_precision") {
        Ok(index) => Some(extract_column::<UInt8Array>(
            cols,
            "size_precision",
            index,
            DataType::UInt8,
        )?),
        Err(_) => None,
    };
    let size_increment_values =
        extract_optional_string_column_by_name(record_batch, "size_increment")?;

    let mut result = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        let id = InstrumentId::from_str(id_values.value(i))
            .map_err(|e| EncodingError::ParseError("id", format!("row {i}: {e}")))?;
        let raw_symbol = Symbol::from(raw_symbol_values.value(i));
        let currency =
            super::decode_currency(currency_values.value(i), "currency", "equity.currency", i)?;
        let price_prec = price_precision_values.value(i);

        let price_increment = Price::from_str(price_increment_values.value(i))
            .map_err(|e| EncodingError::ParseError("price_increment", format!("row {i}: {e}")))?;

        let lot_size = if lot_size_values.is_null(i) {
            None
        } else {
            let lot_size_str = lot_size_values
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    EncodingError::ParseError("lot_size", format!("row {i}: invalid type"))
                })?
                .value(i);
            Some(
                Quantity::from_str(lot_size_str)
                    .map_err(|e| EncodingError::ParseError("lot_size", format!("row {i}: {e}")))?,
            )
        };

        let isin = if isin_values.is_null(i) {
            None
        } else {
            let isin_str = isin_values
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| EncodingError::ParseError("isin", format!("row {i}: invalid type")))?
                .value(i);
            Some(Ustr::from(isin_str))
        };

        let max_quantity =
            if max_quantity_values.is_null(i) {
                None
            } else {
                let max_qty_str = max_quantity_values
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| {
                        EncodingError::ParseError("max_quantity", format!("row {i}: invalid type"))
                    })?
                    .value(i);
                Some(Quantity::from_str(max_qty_str).map_err(|e| {
                    EncodingError::ParseError("max_quantity", format!("row {i}: {e}"))
                })?)
            };

        let min_quantity =
            if min_quantity_values.is_null(i) {
                None
            } else {
                let min_qty_str = min_quantity_values
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| {
                        EncodingError::ParseError("min_quantity", format!("row {i}: invalid type"))
                    })?
                    .value(i);
                Some(Quantity::from_str(min_qty_str).map_err(|e| {
                    EncodingError::ParseError("min_quantity", format!("row {i}: {e}"))
                })?)
            };

        let max_price = if max_price_values.is_null(i) {
            None
        } else {
            let max_p_str = max_price_values
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    EncodingError::ParseError("max_price", format!("row {i}: invalid type"))
                })?
                .value(i);
            Some(
                Price::from_str(max_p_str)
                    .map_err(|e| EncodingError::ParseError("max_price", format!("row {i}: {e}")))?,
            )
        };

        let min_price = if min_price_values.is_null(i) {
            None
        } else {
            let min_p_str = min_price_values
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    EncodingError::ParseError("min_price", format!("row {i}: invalid type"))
                })?
                .value(i);
            Some(
                Price::from_str(min_p_str)
                    .map_err(|e| EncodingError::ParseError("min_price", format!("row {i}: {e}")))?,
            )
        };

        let margin_init = Decimal::from_str(margin_init_values.value(i))
            .map_err(|e| EncodingError::ParseError("margin_init", format!("row {i}: {e}")))?;
        let margin_maint = Decimal::from_str(margin_maint_values.value(i))
            .map_err(|e| EncodingError::ParseError("margin_maint", format!("row {i}: {e}")))?;

        let info = if info_values.is_null(i) {
            None
        } else {
            let info_json = info_values
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| EncodingError::ParseError("info", format!("row {i}: invalid type")))?
                .value(i);

            match serde_json::from_str::<Params>(info_json) {
                Ok(info_dict) => Some(info_dict),
                Err(e) => {
                    return Err(EncodingError::ParseError(
                        "info",
                        format!("row {i}: failed to deserialize JSON: {e}"),
                    ));
                }
            }
        };

        let ts_event = nautilus_core::UnixNanos::from(ts_event_values.value(i));
        let ts_init = nautilus_core::UnixNanos::from(ts_init_values.value(i));

        let tick_scheme = optional_ustr_value(tick_scheme_values, i);

        let size_precision =
            size_precision_values.and_then(|values| (!values.is_null(i)).then(|| values.value(i)));
        let size_increment = size_increment_values
            .and_then(|values| (!values.is_null(i)).then(|| values.value(i)))
            .map(|value| {
                Quantity::from_str(value).map_err(|e| {
                    EncodingError::ParseError("size_increment", format!("row {i}: {e}"))
                })
            })
            .transpose()?;

        let equity = Equity::builder()
            .instrument_id(id)
            .raw_symbol(raw_symbol)
            .maybe_isin(isin)
            .currency(currency)
            .price_precision(price_prec)
            .price_increment(price_increment)
            .maybe_size_precision(size_precision)
            .maybe_size_increment(size_increment)
            .maybe_lot_size(lot_size)
            .maybe_max_quantity(max_quantity)
            .maybe_min_quantity(min_quantity)
            .maybe_max_price(max_price)
            .maybe_min_price(min_price)
            .margin_init(margin_init)
            .margin_maint(margin_maint)
            .maybe_tick_scheme(tick_scheme)
            .maybe_info(info)
            .ts_event(ts_event)
            .ts_init(ts_init)
            .build()
            .map_err(|e| super::instrument_validation_error::<Equity>(i, e))?;

        result.push(equity);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use arrow::{datatypes::Schema, record_batch::RecordBatch};
    use nautilus_model::{
        identifiers::{InstrumentId, Symbol},
        instruments::{Instrument, equity::Equity},
        types::{currency::Currency, price::Price, quantity::Quantity},
    };
    use rstest::rstest;

    use super::decode_equity_batch;
    use crate::arrow::EncodeToRecordBatch;

    fn equity(size: Option<(u8, &str)>) -> Equity {
        let builder = Equity::builder()
            .instrument_id(InstrumentId::from("AAC.XNYS"))
            .raw_symbol(Symbol::from("AAC"))
            .currency(Currency::USD())
            .price_precision(4)
            .price_increment(Price::from("0.0001"))
            .ts_event(1.into())
            .ts_init(2.into());
        match size {
            Some((precision, increment)) => builder
                .size_precision(precision)
                .size_increment(Quantity::from(increment))
                .build(),
            None => builder.build(),
        }
        .unwrap()
    }

    fn round_trip(equity: &Equity) -> Equity {
        let metadata = HashMap::new();
        let batch = Equity::encode_batch(&metadata, std::slice::from_ref(equity)).unwrap();
        decode_equity_batch(&metadata, &batch).unwrap().remove(0)
    }

    #[rstest]
    fn test_fractional_sizes_round_trip() {
        let original = equity(Some((5, "0.00001")));
        let decoded = round_trip(&original);
        assert_eq!(decoded.size_precision(), 5);
        assert_eq!(decoded.size_increment(), Quantity::from("0.00001"));
        assert_eq!(decoded.size_increment().precision, 5);
        assert_eq!(format!("{decoded:?}"), format!("{original:?}"));
    }

    #[rstest]
    fn test_default_sizes_round_trip() {
        let decoded = round_trip(&equity(None));
        assert_eq!(decoded.size_precision(), 0);
        assert_eq!(decoded.size_increment(), Quantity::from(1));
    }

    #[rstest]
    fn test_batch_without_size_columns_decodes_with_defaults() {
        let metadata = HashMap::new();
        let batch = Equity::encode_batch(&metadata, &[equity(Some((5, "0.00001")))]).unwrap();
        // Drop the two appended columns: the schema of batches written before they existed.
        let keep = batch.num_columns() - 2;
        let schema = Schema::new_with_metadata(
            batch.schema().fields()[..keep].to_vec(),
            batch.schema().metadata().clone(),
        );
        let old = RecordBatch::try_new(Arc::new(schema), batch.columns()[..keep].to_vec()).unwrap();
        let decoded = decode_equity_batch(&metadata, &old).unwrap().remove(0);
        assert_eq!(decoded.size_precision(), 0);
        assert_eq!(decoded.size_increment(), Quantity::from(1));
        assert_eq!(decoded.size_increment().precision, 0);
    }

    #[rstest]
    fn test_batch_with_null_size_columns_decodes_with_defaults() {
        use arrow::array::{StringArray, UInt8Array};

        let metadata = HashMap::new();
        let batch = Equity::encode_batch(&metadata, &[equity(Some((5, "0.00001")))]).unwrap();
        let mut columns = batch.columns().to_vec();
        let n = columns.len();
        columns[n - 2] = Arc::new(UInt8Array::from(vec![None::<u8>]));
        columns[n - 1] = Arc::new(StringArray::from(vec![None::<&str>]));
        let nulls = RecordBatch::try_new(batch.schema(), columns).unwrap();
        let decoded = decode_equity_batch(&metadata, &nulls).unwrap().remove(0);
        assert_eq!(decoded.size_precision(), 0);
        assert_eq!(decoded.size_increment(), Quantity::from(1));
    }
}
