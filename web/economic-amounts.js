export function amountText(value) {
  if (value == null) return "—";
  if (typeof value === "object") return String(value.value ?? value.amount ?? "—");
  return String(value);
}

function decimalParts(value) {
  const text = amountText(value).trim();
  const match = /^([+-]?)(\d+)(?:\.(\d+))?$/.exec(text);
  if (!match) return null;
  const scale = match[3]?.length ?? 0;
  const magnitude = BigInt(`${match[2]}${match[3] ?? ""}`);
  return { coefficient: match[1] === "-" ? -magnitude : magnitude, scale };
}

export function decimalDifference(right, left) {
  const rightParts = decimalParts(right);
  const leftParts = decimalParts(left);
  if (!rightParts || !leftParts) return null;
  const scale = Math.max(rightParts.scale, leftParts.scale);
  const factor = 10n ** BigInt(scale);
  const result = rightParts.coefficient * (factor / (10n ** BigInt(rightParts.scale)))
    - leftParts.coefficient * (factor / (10n ** BigInt(leftParts.scale)));
  const negative = result < 0n;
  const absolute = (negative ? -result : result).toString().padStart(scale + 1, "0");
  if (!scale) return `${negative ? "-" : ""}${absolute}`;
  const integer = absolute.slice(0, -scale);
  const fraction = absolute.slice(-scale).replace(/0+$/, "");
  return `${negative ? "-" : ""}${integer}${fraction ? `.${fraction}` : ""}`;
}

export function integerDifference(right, left) {
  const toBigInt = (value) => {
    if (typeof value === "number") return Number.isSafeInteger(value) ? BigInt(value) : null;
    if (typeof value === "string" && /^-?\d+$/.test(value)) return BigInt(value);
    return null;
  };
  const rightValue = toBigInt(right);
  const leftValue = toBigInt(left);
  return rightValue === null || leftValue === null ? null : String(rightValue - leftValue);
}
