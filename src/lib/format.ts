export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx++;
  }
  const decimals = unitIdx >= 3 ? 2 : unitIdx === 2 ? 1 : 0;
  return `${value.toFixed(decimals)} ${units[unitIdx]}`;
}

export function formatRate(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "0 B/s";
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let value = bps;
  let unitIdx = 0;
  while (value >= 1024 && unitIdx < units.length - 1) {
    value /= 1024;
    unitIdx++;
  }
  const decimals = unitIdx >= 2 ? 1 : 0;
  return `${value.toFixed(decimals)} ${units[unitIdx]}`;
}

export function cleanProcessLabel(name: string): string {
  return name.replace(/\.exe$/i, "");
}
