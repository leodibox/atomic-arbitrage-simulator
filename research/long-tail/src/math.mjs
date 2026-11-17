export const E18=10n**18n;
export function isqrt(n){if(n<0n)throw Error('negative sqrt');if(n<2n)return n;let x=n,y=(x+1n)/2n;while(y<x){x=y;y=(x+n/x)/2n;}return x;}
export function ceil(n,d){return (n+d-1n)/d;}
export function v2(x,ri,ro){if(x<=0n||ri<=0n||ro<=0n)throw Error('nonpositive input/liquidity');return x*997n*ro/(1000n*ri+997n*x);}
export function roundtrip(x,a,b){const y=v2(x,a.ri,a.ro);return y===0n?0n:v2(y,b.ri,b.ro);}
export function marginal(a,b){return {numerator:997n**2n*a.ro*b.ro,denominator:1000n**2n*a.ri*b.ri};}
export function net(x,a,b,premiumBps,gas){return roundtrip(x,a,b)-x-ceil(x*premiumBps,10000n)-gas;}
// Continuous static-V2 stationary point, then discrete neighbor checks. Not valid for FOT/rebase or V3.
export function v2Sizing(a,b,premiumBps,gas,cap){
 if(cap<=0n||premiumBps<0n||gas<0n)throw Error('invalid assumptions');
 const A=997n**2n*a.ro*b.ro,B=1000n**2n*a.ri*b.ri,C=1000n*997n*b.ri+997n**2n*a.ro;
 const q=10000n+premiumBps;
 if(A*10000n<=B*q)return {reason:'NEGATIVE_MARGINAL',optimalRaw:0n,bestNetRaw:null,breakEven:null};
 const root=isqrt(A*B*10000n/q);let x=root>B?(root-B)/C:0n;x=x>cap?cap:x;
 let best=null,optimal=0n;
 for(let k=-4n;k<=4n;k++){const t=x+k;if(t<=0n||t>cap)continue;const z=net(t,a,b,premiumBps,gas);if(best===null||z>best){best=z;optimal=t;}}
 const aa=q*C,bb=q*B+gas*10000n*C-A*10000n,cc=gas*10000n*B,D=bb*bb-4n*aa*cc;
 let breakEven=null;
 if(best!==null&&best>0n&&D>=0n&&bb<0n){const d=isqrt(D);breakEven={continuousLowerRaw:(-bb-d)/(2n*aa),continuousUpperRaw:(-bb+d)/(2n*aa),integerRoundingVerified:false};}
 return {reason:best>0n?'POSITIVE_MODEL':'NO_POSITIVE_NET',optimalRaw:optimal,bestNetRaw:best,breakEven,optimality:'continuous stationary point + integer neighbors; rounding plateau/global discrete optimum not proven'};
}
export function gasScenarios(gasUnits,marginWei,failGasUnits=gasUnits){
 return [10n,30n,60n].map(gwei=>{const price=gwei*10n**9n,successCost=gasUnits*price,failureCost=failGasUnits*price;
 const denominator=marginWei-successCost+failureCost;
 const formalThreshold=denominator>0n?ceil(failureCost*1000000n,denominator):null;
 const attainable=formalThreshold!==null&&formalThreshold<=1000000n;
 return {gasPriceGwei:gwei,assumedGasUnits:gasUnits,postGasWei:marginWei-successCost,failCostWei:failureCost,
  ev:[0,25,50,75,100].map(p=>({successProbabilityPercent:p,expectedWei:(BigInt(p)*(marginWei-successCost)-BigInt(100-p)*failureCost)/100n})),
  breakEvenSuccessProbabilityPpm:attainable?formalThreshold:null,breakEvenProbabilityStatus:attainable?'SCENARIO_THRESHOLD_NOT_EMPIRICAL':'UNATTAINABLE_AT_ANY_VALID_PROBABILITY',
  gas_verified:false,executable:false};});
}
export const plain=x=>JSON.parse(JSON.stringify(x,(_,v)=>typeof v==='bigint'?v.toString():v));
