// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script} from "forge-std/Script.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";

/// Deploys the guarded executor. It ships paused, with no loan caps, no
/// approved pairs and no approved V3 pools, so a successful deploy is inert
/// until the owner configures it. Run with:
///   forge script script/DeployGuarded.s.sol --rpc-url mainnet --broadcast
contract DeployGuarded is Script {
    // Aave V3 mainnet pool.
    address constant AAVE = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    // Treasury that receives surplus; never the signer hot key.
    address constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;

    function run() external returns (FlashLoanArbGuarded deployed) {
        uint256 key = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(key);
        deployed = new FlashLoanArbGuarded(AAVE, RECIPIENT);
        vm.stopBroadcast();

        require(deployed.paused(), "deployed executor must start paused");
        require(deployed.profitRecipient() == RECIPIENT, "wrong recipient");
        require(deployed.autoPayout() == false, "auto payout must start off");
    }
}