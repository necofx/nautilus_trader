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

use std::hash::{Hash, Hasher};

use nautilus_core::{
    Params, UnixNanos,
    correctness::{
        CorrectnessError, CorrectnessResult, check_equal_u8, check_valid_string_ascii_optional,
    },
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use super::{Instrument, any::InstrumentAny, tick_scheme::check_tick_scheme};
use crate::{
    enums::{AssetClass, InstrumentClass, OptionKind},
    identifiers::{InstrumentId, Symbol},
    types::{
        currency::Currency,
        money::Money,
        price::{Price, check_positive_price},
        quantity::{Quantity, check_positive_quantity},
    },
};

/// Represents a generic equity instrument.
#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.model", from_py_object)
)]
#[cfg_attr(
    feature = "python",
    pyo3_stub_gen::derive::gen_stub_pyclass(module = "nautilus_trader.model")
)]
pub struct Equity {
    /// The instrument ID.
    pub id: InstrumentId,
    /// The raw/local/native symbol for the instrument, assigned by the venue.
    pub raw_symbol: Symbol,
    /// The instruments International Securities Identification Number (ISIN).
    pub isin: Option<Ustr>,
    /// The futures contract currency.
    pub currency: Currency,
    /// The price decimal precision.
    pub price_precision: u8,
    /// The minimum price increment (tick size).
    pub price_increment: Price,
    /// The trading size decimal precision (default 0: whole shares).
    #[serde(
        default = "default_size_precision",
        skip_serializing_if = "is_default_size_precision"
    )]
    pub size_precision: u8,
    /// The minimum size increment (default 1: whole shares).
    #[serde(
        default = "default_size_increment",
        skip_serializing_if = "is_default_size_increment"
    )]
    pub size_increment: Quantity,
    /// The initial (order) margin requirement in percentage of order value.
    pub margin_init: Decimal,
    /// The maintenance (position) margin in percentage of position value.
    pub margin_maint: Decimal,
    /// The rounded lot unit size (standard/board).
    pub lot_size: Option<Quantity>,
    /// The maximum allowable order quantity.
    pub max_quantity: Option<Quantity>,
    /// The minimum allowable order quantity.
    pub min_quantity: Option<Quantity>,
    /// The maximum allowable quoted price.
    pub max_price: Option<Price>,
    /// The minimum allowable quoted price.
    pub min_price: Option<Price>,
    /// The registered variable tick scheme name.
    pub tick_scheme: Option<Ustr>,
    /// Additional instrument metadata as a JSON-serializable dictionary.
    pub info: Option<Params>,
    /// UNIX timestamp (nanoseconds) when the data event occurred.
    pub ts_event: UnixNanos,
    /// UNIX timestamp (nanoseconds) when the data object was initialized.
    pub ts_init: UnixNanos,
}

const fn default_size_precision() -> u8 {
    0
}

fn default_size_increment() -> Quantity {
    Quantity::from(1)
}

const fn is_default_size_precision(size_precision: &u8) -> bool {
    *size_precision == 0
}

fn is_default_size_increment(size_increment: &Quantity) -> bool {
    size_increment.precision == 0 && *size_increment == default_size_increment()
}

#[bon::bon]
impl Equity {
    #[expect(clippy::too_many_arguments)]
    fn new_checked(
        instrument_id: InstrumentId,
        raw_symbol: Symbol,
        isin: Option<Ustr>,
        currency: Currency,
        price_precision: u8,
        price_increment: Price,
        lot_size: Option<Quantity>,
        max_quantity: Option<Quantity>,
        min_quantity: Option<Quantity>,
        max_price: Option<Price>,
        min_price: Option<Price>,
        margin_init: Option<Decimal>,
        margin_maint: Option<Decimal>,
        tick_scheme: Option<Ustr>,
        info: Option<Params>,
        ts_event: UnixNanos,
        ts_init: UnixNanos,
    ) -> CorrectnessResult<Self> {
        check_valid_string_ascii_optional(isin, stringify!(isin))?;
        check_equal_u8(
            price_precision,
            price_increment.precision,
            stringify!(price_precision),
            stringify!(price_increment.precision),
        )?;
        check_positive_price(price_increment, stringify!(price_increment))?;
        check_tick_scheme(tick_scheme)?;

        if let Some(lot_size) = lot_size {
            check_positive_quantity(lot_size, stringify!(lot_size))?;
        }

        Ok(Self {
            id: instrument_id,
            raw_symbol,
            isin,
            currency,
            price_precision,
            price_increment,
            size_precision: default_size_precision(),
            size_increment: default_size_increment(),
            lot_size,
            max_quantity,
            min_quantity,
            max_price,
            min_price,
            margin_init: margin_init.unwrap_or_default(),
            margin_maint: margin_maint.unwrap_or_default(),
            tick_scheme,
            info,
            ts_event,
            ts_init,
        })
    }

    /// Returns this equity with a fractional (or any non-default) trading size.
    ///
    /// By default an equity trades whole shares (`size_precision` 0, `size_increment` 1). Some
    /// venues trade fractional shares, and split back-adjusted historical data carries
    /// fractional volumes, which bars can only hold at a matching size precision.
    ///
    /// # Errors
    ///
    /// Returns an error if `size_increment.precision` differs from `size_precision`, if
    /// `size_increment` is not positive, or if `lot_size`, `min_quantity` or `max_quantity` (when
    /// set) has another precision than `size_precision`.
    pub fn with_size(
        mut self,
        size_precision: u8,
        size_increment: Quantity,
    ) -> CorrectnessResult<Self> {
        check_equal_u8(
            size_precision,
            size_increment.precision,
            stringify!(size_precision),
            stringify!(size_increment.precision),
        )?;
        check_positive_quantity(size_increment, stringify!(size_increment))?;
        for (quantity, param) in [
            (self.lot_size, "lot_size.precision"),
            (self.min_quantity, "min_quantity.precision"),
            (self.max_quantity, "max_quantity.precision"),
        ] {
            if let Some(quantity) = quantity {
                check_equal_u8(
                    quantity.precision,
                    size_precision,
                    param,
                    stringify!(size_precision),
                )?;
            }
        }
        self.size_precision = size_precision;
        self.size_increment = size_increment;
        Ok(self)
    }

    /// Returns a fluent builder for a [`Equity`] instance.
    ///
    /// Required fields are enforced at compile time; optional fields can be omitted and use the
    /// same defaults as checked construction. The same correctness checks run on `build`.
    /// `size_precision` and `size_increment` are optional and go together (see
    /// [`Equity::with_size`]); without them the equity trades whole shares.
    ///
    /// # Errors
    ///
    /// Returns an error if any input validation fails, or if only one of `size_precision` and
    /// `size_increment` is given.
    #[builder(start_fn = builder, finish_fn = build)]
    pub fn build_checked(
        instrument_id: InstrumentId,
        raw_symbol: Symbol,
        isin: Option<Ustr>,
        currency: Currency,
        price_precision: u8,
        price_increment: Price,
        size_precision: Option<u8>,
        size_increment: Option<Quantity>,
        lot_size: Option<Quantity>,
        max_quantity: Option<Quantity>,
        min_quantity: Option<Quantity>,
        max_price: Option<Price>,
        min_price: Option<Price>,
        margin_init: Option<Decimal>,
        margin_maint: Option<Decimal>,
        tick_scheme: Option<Ustr>,
        info: Option<Params>,
        ts_event: UnixNanos,
        ts_init: UnixNanos,
    ) -> CorrectnessResult<Self> {
        let equity = Self::new_checked(
            instrument_id,
            raw_symbol,
            isin,
            currency,
            price_precision,
            price_increment,
            lot_size,
            max_quantity,
            min_quantity,
            max_price,
            min_price,
            margin_init,
            margin_maint,
            tick_scheme,
            info,
            ts_event,
            ts_init,
        )?;
        match (size_precision, size_increment) {
            (None, None) => Ok(equity),
            (Some(size_precision), Some(size_increment)) => {
                equity.with_size(size_precision, size_increment)
            }
            _ => Err(CorrectnessError::PredicateViolation {
                message: "`size_precision` and `size_increment` must be given together".to_string(),
            }),
        }
    }
}

impl PartialEq<Self> for Equity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Equity {}

impl Hash for Equity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Instrument for Equity {
    fn into_any(self) -> InstrumentAny {
        InstrumentAny::Equity(self)
    }

    fn id(&self) -> InstrumentId {
        self.id
    }

    fn raw_symbol(&self) -> Symbol {
        self.raw_symbol
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Equity
    }

    fn instrument_class(&self) -> InstrumentClass {
        InstrumentClass::Spot
    }
    fn underlying(&self) -> Option<Ustr> {
        None
    }

    fn base_currency(&self) -> Option<Currency> {
        None
    }

    fn quote_currency(&self) -> Currency {
        self.currency
    }

    fn settlement_currency(&self) -> Currency {
        self.currency
    }

    fn isin(&self) -> Option<Ustr> {
        self.isin
    }

    fn option_kind(&self) -> Option<OptionKind> {
        None
    }

    fn exchange(&self) -> Option<Ustr> {
        None
    }

    fn strike_price(&self) -> Option<Price> {
        None
    }

    fn activation_ns(&self) -> Option<UnixNanos> {
        None
    }

    fn expiration_ns(&self) -> Option<UnixNanos> {
        None
    }

    fn is_inverse(&self) -> bool {
        false
    }

    fn price_precision(&self) -> u8 {
        self.price_precision
    }

    fn size_precision(&self) -> u8 {
        self.size_precision
    }

    fn price_increment(&self) -> Price {
        self.price_increment
    }

    fn size_increment(&self) -> Quantity {
        self.size_increment
    }

    fn multiplier(&self) -> Quantity {
        Quantity::from(1)
    }

    fn lot_size(&self) -> Option<Quantity> {
        self.lot_size
    }

    fn max_quantity(&self) -> Option<Quantity> {
        self.max_quantity
    }

    fn min_quantity(&self) -> Option<Quantity> {
        self.min_quantity
    }

    fn max_notional(&self) -> Option<Money> {
        None
    }

    fn min_notional(&self) -> Option<Money> {
        None
    }

    fn max_price(&self) -> Option<Price> {
        self.max_price
    }

    fn min_price(&self) -> Option<Price> {
        self.min_price
    }

    fn tick_scheme(&self) -> Option<Ustr> {
        self.tick_scheme
    }

    fn info(&self) -> Option<&Params> {
        self.info.as_ref()
    }

    fn ts_event(&self) -> UnixNanos {
        self.ts_event
    }

    fn ts_init(&self) -> UnixNanos {
        self.ts_init
    }

    fn margin_init(&self) -> Decimal {
        self.margin_init
    }

    fn margin_maint(&self) -> Decimal {
        self.margin_maint
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use rust_decimal_macros::dec;
    use ustr::Ustr;

    use crate::{
        enums::{AssetClass, InstrumentClass},
        identifiers::{InstrumentId, Symbol},
        instruments::{Equity, Instrument, stubs::*},
        types::{Currency, Price, Quantity},
    };

    #[rstest]
    fn test_trait_accessors(equity_aapl: Equity) {
        assert_eq!(equity_aapl.id(), InstrumentId::from("AAPL.XNAS"));
        assert_eq!(equity_aapl.raw_symbol(), Symbol::from("AAPL"));
        assert_eq!(equity_aapl.asset_class(), AssetClass::Equity);
        assert_eq!(equity_aapl.instrument_class(), InstrumentClass::Spot);
        assert_eq!(equity_aapl.quote_currency(), Currency::USD());
        assert_eq!(equity_aapl.settlement_currency(), Currency::USD());
        assert!(!equity_aapl.is_inverse());
        assert_eq!(equity_aapl.price_precision(), 2);
        assert_eq!(equity_aapl.size_precision(), 0);
        assert_eq!(equity_aapl.price_increment(), Price::from("0.01"));
        assert_eq!(equity_aapl.size_increment(), Quantity::from("1"));
        assert_eq!(equity_aapl.multiplier(), Quantity::from("1"));
        assert_eq!(equity_aapl.base_currency(), None);
        assert_eq!(equity_aapl.underlying(), None);
        assert_eq!(equity_aapl.option_kind(), None);
        assert_eq!(equity_aapl.strike_price(), None);
        assert_eq!(equity_aapl.activation_ns(), None);
        assert_eq!(equity_aapl.expiration_ns(), None);
    }

    #[rstest]
    fn test_isin(equity_aapl: Equity) {
        assert_eq!(
            equity_aapl.isin().map(|u| u.to_string()),
            Some("US0378331005".to_string()),
        );
    }

    #[rstest]
    fn test_new_checked_price_precision_mismatch() {
        let result = Equity::new_checked(
            InstrumentId::from("AAPL.XNAS"),
            Symbol::from("AAPL"),
            None,
            Currency::USD(),
            3, // mismatch
            Price::from("0.01"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0.into(),
            0.into(),
        );
        assert!(result.is_err());
    }

    #[rstest]
    fn test_new_checked_zero_price_increment() {
        let result = Equity::new_checked(
            InstrumentId::from("AAPL.XNAS"),
            Symbol::from("AAPL"),
            None,
            Currency::USD(),
            0,
            Price::from("0"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0.into(),
            0.into(),
        );
        assert!(result.is_err());
    }

    #[rstest]
    fn test_new_checked_non_ascii_isin() {
        let result = Equity::new_checked(
            InstrumentId::from("AAPL.XNAS"),
            Symbol::from("AAPL"),
            Some(ustr::Ustr::from("US\u{00E9}378331005")),
            Currency::USD(),
            2,
            Price::from("0.01"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0.into(),
            0.into(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-ASCII"));
    }

    #[rstest]
    fn test_new_checked_rejects_non_positive_lot_size() {
        let result = Equity::new_checked(
            InstrumentId::from("AAPL.XNAS"),
            Symbol::from("AAPL"),
            None,
            Currency::USD(),
            2,
            Price::from("0.01"),
            Some(Quantity::from("0")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0.into(),
            0.into(),
        );
        let error = result.unwrap_err();
        assert!(error.to_string().contains("not positive"), "{error}");
    }

    #[rstest]
    fn test_serialization_roundtrip(equity_aapl: Equity) {
        let json = serde_json::to_string(&equity_aapl).unwrap();
        let deserialized: Equity = serde_json::from_str(&json).unwrap();
        assert_eq!(json, serde_json::to_string(&deserialized).unwrap());
    }

    #[rstest]
    fn test_builder_matches_new_checked() {
        let positional = Equity::new_checked(
            InstrumentId::from("AAPL.XNAS"),
            Symbol::from("AAPL"),
            Some(Ustr::from("US0378331005")),
            Currency::USD(),
            2,
            Price::from("0.01"),
            Some(Quantity::from("100")),
            Some(Quantity::from("10000.0")),
            Some(Quantity::from("0.001")),
            Some(Price::from("9999.99")),
            Some(Price::from("0.01")),
            Some(dec!(0.01)),
            Some(dec!(0.02)),
            None,
            None,
            1.into(),
            2.into(),
        )
        .unwrap();

        let built = Equity::builder()
            .instrument_id(InstrumentId::from("AAPL.XNAS"))
            .raw_symbol(Symbol::from("AAPL"))
            .isin(Ustr::from("US0378331005"))
            .currency(Currency::USD())
            .price_precision(2)
            .price_increment(Price::from("0.01"))
            .lot_size(Quantity::from("100"))
            .max_quantity(Quantity::from("10000.0"))
            .min_quantity(Quantity::from("0.001"))
            .max_price(Price::from("9999.99"))
            .min_price(Price::from("0.01"))
            .margin_init(dec!(0.01))
            .margin_maint(dec!(0.02))
            .ts_event(1.into())
            .ts_init(2.into())
            .build()
            .unwrap();

        assert_eq!(
            serde_json::to_value(&positional).unwrap(),
            serde_json::to_value(&built).unwrap(),
        );
    }

    fn fractional_builder() -> Equity {
        Equity::builder()
            .instrument_id(InstrumentId::from("AAPL.XNAS"))
            .raw_symbol(Symbol::from("AAPL"))
            .currency(Currency::USD())
            .price_precision(2)
            .price_increment(Price::from("0.01"))
            .size_precision(5)
            .size_increment(Quantity::from("0.00001"))
            .lot_size(Quantity::from("1.00000"))
            .ts_event(0.into())
            .ts_init(0.into())
            .build()
            .unwrap()
    }

    #[rstest]
    fn test_default_sizes_are_whole_shares(equity_aapl: Equity) {
        assert_eq!(equity_aapl.size_precision, 0);
        assert_eq!(equity_aapl.size_increment, Quantity::from(1));
        assert_eq!(equity_aapl.size_increment.precision, 0);
    }

    #[rstest]
    fn test_with_size_sets_fractional_sizes(equity_aapl: Equity) {
        let equity = equity_aapl.with_size(5, Quantity::from("0.00001")).unwrap();
        assert_eq!(equity.size_precision(), 5);
        assert_eq!(equity.size_increment(), Quantity::from("0.00001"));
        assert_eq!(equity.size_increment().precision, 5);
    }

    #[rstest]
    fn test_builder_with_fractional_sizes() {
        let equity = fractional_builder();
        assert_eq!(equity.size_precision(), 5);
        assert_eq!(equity.size_increment(), Quantity::from("0.00001"));
        assert_eq!(equity.lot_size(), Some(Quantity::from("1.00000")));
        assert_eq!(equity.multiplier(), Quantity::from(1));
    }

    #[rstest]
    fn test_with_size_rejects_precision_mismatch() {
        let error = fractional_builder()
            .with_size(4, Quantity::from("0.00001"))
            .unwrap_err();
        assert!(error.to_string().contains("size_precision"), "{error}");
    }

    #[rstest]
    fn test_with_size_rejects_zero_increment() {
        let error = fractional_builder()
            .with_size(5, Quantity::from("0.00000"))
            .unwrap_err();
        assert!(error.to_string().contains("not positive"), "{error}");
    }

    #[rstest]
    fn test_with_size_rejects_lot_size_at_another_precision() {
        let error = fractional_builder()
            .with_size(3, Quantity::from("0.001"))
            .unwrap_err();
        assert!(error.to_string().contains("lot_size.precision"), "{error}");
    }

    #[rstest]
    fn test_builder_rejects_size_precision_without_increment() {
        let result = Equity::builder()
            .instrument_id(InstrumentId::from("AAPL.XNAS"))
            .raw_symbol(Symbol::from("AAPL"))
            .currency(Currency::USD())
            .price_precision(2)
            .price_increment(Price::from("0.01"))
            .size_precision(5)
            .ts_event(0.into())
            .ts_init(0.into())
            .build();
        let error = result.unwrap_err();
        assert!(error.to_string().contains("together"), "{error}");
    }

    #[rstest]
    fn test_serialization_omits_default_sizes(equity_aapl: Equity) {
        let json = serde_json::to_string(&equity_aapl).unwrap();
        assert!(!json.contains("size_precision"), "{json}");
        assert!(!json.contains("size_increment"), "{json}");
    }

    #[rstest]
    fn test_serialization_roundtrip_fractional_sizes() {
        let equity = fractional_builder();
        let json = serde_json::to_string(&equity).unwrap();
        assert!(json.contains("\"size_precision\":5"), "{json}");
        let deserialized: Equity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.size_precision, 5);
        assert_eq!(deserialized.size_increment, Quantity::from("0.00001"));
        assert_eq!(deserialized.size_increment.precision, 5);
    }
}
