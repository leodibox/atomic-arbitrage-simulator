// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {Sandooo} from "../src/Sandooo.sol";

interface IPair { function getReserves() external view returns (uint112, uint112, uint32); }

contract SandoooTest is Test {
    Sandooo s;
    address pair = 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc;
    address weth = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

    function setUp() public {
        vm.createSelectFork(vm.envString("HTTPS_URL"));
        s = new Sandooo();
    }

    function test_staleBlockReverts() public {
        bytes memory bad = abi.encodePacked(uint64(block.number - 1));
        vm.prank(address(this));
        (bool ok,) = address(s).call(bad);
        assertFalse(ok);
    }
}
