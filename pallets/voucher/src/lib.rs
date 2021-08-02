#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::sp_runtime::{
	traits::{AtLeast32BitUnsigned, Bounded, CheckedAdd, MaybeSerializeDeserialize, One, Zero},
	RuntimeDebug,
};
use frame_support::{traits::BalanceStatus, transactional, Parameter};
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, MultiReservableCurrency};
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
pub use pallet::*;
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[derive(Encode, Decode, Clone, RuntimeDebug, Eq, PartialEq)]
pub struct Voucher<CurrencyId, Balance, AccountId> {
	pub currency_id: CurrencyId,
	#[codec(compact)]
	pub amount: Balance,
	pub owner: AccountId,
	pub valid_merchants: Vec<AccountId>,
	pub redeemable_by: AccountId,
}

type BalanceOf<T> =
	<<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;
type CurrencyIdOf<T> =
	<<T as Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
type VoucherOf<T> = Voucher<CurrencyIdOf<T>, BalanceOf<T>, <T as frame_system::Config>::AccountId>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::DispatchResult, dispatch::DispatchResultWithPostInfo, pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Currency
		type Currency: MultiReservableCurrency<Self::AccountId>;
		/// VoucherId
		type VoucherId: Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ Bounded;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn vouchers)]
	pub type Vouchers<T: Config> = StorageMap<_, Twox64Concat, T::VoucherId, VoucherOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn next_voucherid)]
	pub type NextVoucherId<T: Config> = StorageValue<_, T::VoucherId>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId", T::VoucherId = "VoucherId", VoucherOf<T> = "Voucher", BalanceOf<T> = "Balance")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		VoucherCreated(T::VoucherId, VoucherOf<T>),
		VoucherRedeemed(
			T::AccountId,
			T::VoucherId,
			VoucherOf<T>,
			T::AccountId,
			BalanceOf<T>,
		),
		VoucherCancelled(T::VoucherId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		VoucherIdOverflow,
		InvalidVoucherId,
		InsufficientBalance,
		NotOwner,
		InvalidCustomer,
		InvalidMerchant,
		AmountExceeded,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		// Voucher submitted by the payer
		pub fn submit_voucher(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			amount: BalanceOf<T>,
			valid_merchants: Vec<<T as frame_system::Config>::AccountId>,
			redeemable_by: <T as frame_system::Config>::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			NextVoucherId::<T>::try_mutate(|id| -> DispatchResultWithPostInfo {
				let voucher_id = id.unwrap_or_default();

				let voucher = Voucher {
					currency_id,
					amount,
					valid_merchants,
					redeemable_by,
					owner: who.clone(),
				};

				*id = Some(
					voucher_id
						.checked_add(&One::one())
						.ok_or(Error::<T>::VoucherIdOverflow)?,
				);

				// Reserve the voucher amount
				T::Currency::reserve(currency_id, &who, amount)?;

				Vouchers::<T>::insert(voucher_id, &voucher);

				Self::deposit_event(Event::VoucherCreated(voucher_id, voucher));
				Ok(().into())
			})?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		// Voucher redeemed by customer to pay to merchant
		pub fn redeem_voucher(
			origin: OriginFor<T>,
			voucher_id: T::VoucherId,
			merchant: <T as frame_system::Config>::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Vouchers::<T>::try_mutate_exists(
				voucher_id,
				|voucher| -> DispatchResultWithPostInfo {
					let voucher = voucher.take().ok_or(Error::<T>::InvalidVoucherId)?;

					// Voucher should be redeemed by the intended customer
					ensure!(voucher.redeemable_by == who, Error::<T>::InvalidCustomer);
					// Voucher should be redeemed for a valid merchant
					ensure!(
						voucher.valid_merchants.iter().any(|m| *m == merchant),
						Error::<T>::InvalidMerchant
					);
					// Redeemed amount should not be greater than voucher amount
					ensure!(voucher.amount >= amount, Error::<T>::AmountExceeded);
					let bal = voucher.amount - amount;

					// Transfer the amount from reserved balance of payer to merchant
					let val = T::Currency::repatriate_reserved(
						voucher.currency_id,
						&voucher.owner,
						&merchant,
						amount,
						BalanceStatus::Free,
					)?;
					ensure!(val.is_zero(), Error::<T>::InsufficientBalance);
					// Unreserve unredeemed amount
					T::Currency::unreserve(voucher.currency_id, &voucher.owner, bal);

					Self::deposit_event(Event::VoucherRedeemed(
						who, voucher_id, voucher, merchant, amount,
					));
					Ok(().into())
				},
			)?;
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		// Voucher cancelled by payee
		pub fn cancel_voucher(
			origin: OriginFor<T>,
			voucher_id: T::VoucherId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Vouchers::<T>::try_mutate_exists(voucher_id, |voucher| -> DispatchResult {
				let voucher = voucher.take().ok_or(Error::<T>::InvalidVoucherId)?;

				ensure!(voucher.owner == who, Error::<T>::NotOwner);

				Self::deposit_event(Event::VoucherCancelled(voucher_id));
				Ok(())
			})?;
			Ok(().into())
		}
	}
}
