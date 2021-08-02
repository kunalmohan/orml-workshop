use super::*;
pub use crate::mock::{
    Currencies, Event, ExtBuilder, Origin, System, Tokens, VoucherModule, ALICE, BOB,
};
use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_std::vec::Vec;

const ENDOWED_AMOUNT: u128 = 1_000_000_000_000_000;

fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| System::set_block_number(1));
    ext
}

fn events() -> Vec<Event> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();
    System::reset_events();
    evt
}

#[test]
fn test_submit_voucher() {
    new_test_ext().execute_with(|| {
        //sell amount <= balance
        assert_ok!(VoucherModule::submit_voucher(
            Origin::signed(ALICE),
            DOT,
            10,
            vec![DENIS, CAMEL],
            BOB
        ));

        assert_eq!(Tokens::free_balance(DOT, &ALICE), ENDOWED_AMOUNT - 10);

        assert_eq!(
            events().as_slice(),
            [Event::pallet_voucher(crate::Event::VoucherCreated(
                0,
                Voucher {
                    currency_id: DOT,
                    amount: 10,
                    valid_merchants: vec![DENIS, CAMEL],
                    redeemable_by: BOB,
                    owner: ALICE
                }
            )),]
        );
    });
}

#[test]
fn test_redeem_voucher() {
    new_test_ext().execute_with(|| {
        //id not exist
        assert_noop!(
            VoucherModule::redeem_voucher(Origin::signed(BOB), 0, CAMEL, 8),
            Error::<Test>::InvalidVoucherId
        );

        //id exist
        assert_ok!(VoucherModule::submit_voucher(
            Origin::signed(ALICE),
            DOT,
            10,
            vec![DENIS, CAMEL],
            BOB
        ));
        assert_eq!(Tokens::free_balance(DOT, &ALICE), ENDOWED_AMOUNT - 10);

        //excess redeem amount
        assert_noop!(
            VoucherModule::redeem_voucher(Origin::signed(BOB), 0, CAMEL, 12),
            Error::<Test>::AmountExceeded
        );
        //invalid merchant
        assert_noop!(
            VoucherModule::redeem_voucher(Origin::signed(BOB), 0, FRED, 10),
            Error::<Test>::InvalidMerchant
        );
        //invalid customer/redeemer
        assert_noop!(
            VoucherModule::redeem_voucher(Origin::signed(DENIS), 0, CAMEL, 10),
            Error::<Test>::InvalidCustomer
        );
        assert_ok!(VoucherModule::redeem_voucher(
            Origin::signed(BOB),
            0,
            CAMEL,
            8
        ));
        assert_eq!(Tokens::free_balance(DOT, &ALICE), ENDOWED_AMOUNT - 8);
        assert_eq!(Tokens::free_balance(DOT, &CAMEL), ENDOWED_AMOUNT + 8);

        assert_eq!(
            events().as_slice(),
            [
                Event::pallet_voucher(crate::Event::VoucherCreated(
                    0,
                    Voucher {
                        currency_id: DOT,
                        amount: 10,
                        valid_merchants: vec![DENIS, CAMEL],
                        redeemable_by: BOB,
                        owner: ALICE
                    }
                )),
                Event::pallet_voucher(crate::Event::VoucherRedeemed(
                    BOB,
                    0,
                    Voucher {
                        currency_id: DOT,
                        amount: 10,
                        valid_merchants: vec![DENIS, CAMEL],
                        redeemable_by: BOB,
                        owner: ALICE
                    },
                    CAMEL,
                    8
                ))
            ]
        );
    });
}

#[test]
fn test_cancel_voucher() {
    new_test_ext().execute_with(|| {
        //id not exist
        assert_noop!(
            VoucherModule::cancel_voucher(Origin::signed(ALICE), 0),
            Error::<Test>::InvalidVoucherId
        );

        //id exist, it is not owner
        assert_ok!(VoucherModule::submit_voucher(
            Origin::signed(ALICE),
            DOT,
            10,
            vec![DENIS, CAMEL],
            BOB
        ));

        assert_noop!(
            VoucherModule::cancel_voucher(Origin::signed(BOB), 0),
            Error::<Test>::NotOwner
        );

        //id exist, is owner
        assert_ok!(VoucherModule::cancel_voucher(Origin::signed(ALICE), 0));

        assert_eq!(
            events().as_slice(),
            [
                Event::pallet_voucher(crate::Event::VoucherCreated(
                    0,
                    Voucher {
                        currency_id: DOT,
                        amount: 10,
                        valid_merchants: vec![DENIS, CAMEL],
                        redeemable_by: BOB,
                        owner: ALICE
                    }
                )),
                Event::pallet_voucher(crate::Event::VoucherCancelled(0))
            ]
        );
    });
}
