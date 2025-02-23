// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ERC20} from "./openzeppelin-contracts/contracts/token/ERC20/ERC20.sol";
import { IERC20 } from "./openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "./openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";


contract Gold is ERC20 {
    using SafeERC20 for IERC20;

    constructor() ERC20("Gold", "AU") {
        _mint(msg.sender, 1000   ** decimals());
    }
}
