import { ethers } from "ethers";
const USDC_E = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
const CTF = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
const CTF_EXCHANGE = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";
const NEG_RISK_CTF_EXCHANGE = "0xC5d563A36AE78145C45a50134d48A1215220f80a";
const NEG_RISK_ADAPTER = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";
const iface = new ethers.utils.Interface([
    "function allowance(address owner, address spender) view returns (uint256)",
    "function approve(address spender, uint256 amount) returns (bool)",
    "function setApprovalForAll(address operator, bool approved)",
    "function isApprovedForAll(address account, address operator) view returns (bool)",
]);
function minApprovedAmount() {
    return ethers.constants.MaxUint256.div(2).add(1);
}
export async function walletAddressAndBalance(privateKey, rpcUrl) {
    const provider = new ethers.providers.JsonRpcProvider(rpcUrl);
    const wallet = new ethers.Wallet(privateKey.startsWith("0x") ? privateKey : "0x" + privateKey, provider);
    const address = await wallet.getAddress();
    const balance = await provider.getBalance(address);
    return { address, balance };
}
async function checkUsdcAllowance(provider, owner, usdc, spender) {
    const data = iface.encodeFunctionData("allowance", [owner, spender]);
    const out = await provider.call({
        to: usdc,
        from: owner,
        data,
    });
    return ethers.BigNumber.from(out);
}
async function checkCtfApprovedForAll(provider, account, ctf, operator) {
    const data = iface.encodeFunctionData("isApprovedForAll", [
        account,
        operator,
    ]);
    const out = await provider.call({
        to: ctf,
        from: account,
        data,
    });
    return iface.decodeFunctionResult("isApprovedForAll", out)[0];
}
export async function approveAllowance(privateKey, rpcUrl, includeNegRisk) {
    const provider = new ethers.providers.JsonRpcProvider(rpcUrl);
    const wallet = new ethers.Wallet(privateKey.startsWith("0x") ? privateKey : "0x" + privateKey).connect(provider);
    const from = await wallet.getAddress();
    const minApproved = minApprovedAmount();
    const maxU256 = ethers.constants.MaxUint256;
    const usdc = USDC_E;
    const ctfAddr = CTF;
    const ctfExchangeAddr = CTF_EXCHANGE;
    const allowanceCtf = await checkUsdcAllowance(provider, from, usdc, ctfAddr);
    if (allowanceCtf.lt(minApproved)) {
        const tx = await wallet.sendTransaction({
            to: usdc,
            data: iface.encodeFunctionData("approve", [ctfAddr, maxU256]),
            gasLimit: 100_000,
        });
        const receipt = await tx.wait();
        console.log("USDC approve(CTF) tx:", receipt.transactionHash, "success=", receipt.status === 1);
    }
    else {
        console.log("USDC allowance(CTF) already sufficient");
    }
    const ctfOk = await checkCtfApprovedForAll(provider, from, ctfAddr, ctfExchangeAddr);
    if (!ctfOk) {
        const tx2 = await wallet.sendTransaction({
            to: ctfAddr,
            data: iface.encodeFunctionData("setApprovalForAll", [
                ctfExchangeAddr,
                true,
            ]),
            gasLimit: 100_000,
        });
        const receipt2 = await tx2.wait();
        console.log("CTF setApprovalForAll(CTF_EXCHANGE) tx:", receipt2.transactionHash, "success=", receipt2.status === 1);
    }
    else {
        console.log("CTF already setApprovalForAll(CTF_EXCHANGE)");
    }
    if (includeNegRisk) {
        const negCtf = NEG_RISK_CTF_EXCHANGE;
        const negAdapter = NEG_RISK_ADAPTER;
        const allowNegCtf = await checkUsdcAllowance(provider, from, usdc, negCtf);
        if (allowNegCtf.lt(minApproved)) {
            const tx3 = await wallet.sendTransaction({
                to: usdc,
                data: iface.encodeFunctionData("approve", [negCtf, maxU256]),
                gasLimit: 100_000,
            });
            const r3 = await tx3.wait();
            console.log("USDC approve(NEG_RISK_CTF_EXCHANGE) tx:", r3.transactionHash);
        }
        const ctfNegOk = await checkCtfApprovedForAll(provider, from, ctfAddr, negCtf);
        if (!ctfNegOk) {
            const tx4 = await wallet.sendTransaction({
                to: ctfAddr,
                data: iface.encodeFunctionData("setApprovalForAll", [negCtf, true]),
                gasLimit: 100_000,
            });
            const r4 = await tx4.wait();
            console.log("CTF setApprovalForAll(NEG_RISK_CTF_EXCHANGE) tx:", r4.transactionHash);
        }
        const allowAdapter = await checkUsdcAllowance(provider, from, usdc, negAdapter);
        if (allowAdapter.lt(minApproved)) {
            const tx5 = await wallet.sendTransaction({
                to: usdc,
                data: iface.encodeFunctionData("approve", [negAdapter, maxU256]),
                gasLimit: 100_000,
            });
            const r5 = await tx5.wait();
            console.log("USDC approve(NEG_RISK_ADAPTER) tx:", r5.transactionHash);
        }
        const ctfAdapterOk = await checkCtfApprovedForAll(provider, from, ctfAddr, negAdapter);
        if (!ctfAdapterOk) {
            const tx6 = await wallet.sendTransaction({
                to: ctfAddr,
                data: iface.encodeFunctionData("setApprovalForAll", [
                    negAdapter,
                    true,
                ]),
                gasLimit: 100_000,
            });
            const r6 = await tx6.wait();
            console.log("CTF setApprovalForAll(NEG_RISK_ADAPTER) tx:", r6.transactionHash);
        }
    }
    console.log("Allowance check/approval done");
}
