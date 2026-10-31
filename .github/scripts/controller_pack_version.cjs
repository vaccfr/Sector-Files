function parseVersion(value) {
  const normalized = String(value || "")
    .trim()
    .replace(/^release[-_/]/i, "")
    .replace(/^v/i, "");
  const match = /^(\d+(?:\.\d+)*)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(normalized);
  if (!match) return null;

  const numbers = match[1].split(".").map(Number);
  if (numbers.some((number) => !Number.isSafeInteger(number))) return null;
  const prerelease = match[2] ? match[2].split(".") : [];
  if (prerelease.some((part) => !part)) return null;
  return { numbers, prerelease };
}

function compareVersions(left, right) {
  const a = parseVersion(left);
  const b = parseVersion(right);
  if (!a || !b) return null;

  for (let index = 0; index < Math.max(a.numbers.length, b.numbers.length); index++) {
    const difference = (a.numbers[index] || 0) - (b.numbers[index] || 0);
    if (difference) return Math.sign(difference);
  }

  if (!a.prerelease.length && b.prerelease.length) return 1;
  if (a.prerelease.length && !b.prerelease.length) return -1;
  for (let index = 0; index < Math.min(a.prerelease.length, b.prerelease.length); index++) {
    const leftPart = a.prerelease[index];
    const rightPart = b.prerelease[index];
    const leftNumeric = /^\d+$/.test(leftPart);
    const rightNumeric = /^\d+$/.test(rightPart);
    if (leftNumeric && rightNumeric) {
      const leftValue = BigInt(leftPart);
      const rightValue = BigInt(rightPart);
      if (leftValue !== rightValue) return leftValue > rightValue ? 1 : -1;
    } else if (leftNumeric !== rightNumeric) {
      return leftNumeric ? -1 : 1;
    } else if (leftPart !== rightPart) {
      return leftPart > rightPart ? 1 : -1;
    }
  }
  return Math.sign(a.prerelease.length - b.prerelease.length);
}

module.exports = { compareVersions };
