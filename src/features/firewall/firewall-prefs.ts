const PREFER_SUBNET_KEY = 'queryben.firewall.preferSubnet';

export function loadPreferSubnet(): boolean {
  try {
    return window.localStorage.getItem(PREFER_SUBNET_KEY) === 'true';
  } catch {
    return false;
  }
}

export function savePreferSubnet(value: boolean): void {
  try {
    window.localStorage.setItem(PREFER_SUBNET_KEY, value ? 'true' : 'false');
  } catch {
    // quota / private mode; preference won't survive the session
  }
}

// ISO timestamp normalized to dashes so the rule name is safe in ARM resource paths.
export function generateRuleName(isSubnet: boolean): string {
  const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  return `QueryBen${isSubnet ? '_subnet' : ''}_${ts}`;
}

// Expand an IPv4 address into its /24 range. IPv6 not handled —
// Azure SQL firewall is IPv4-only and 40615 payloads only carry v4.
export function toSubnetRange(ip: string): { start: string; end: string } {
  const parts = ip.split('.');
  if (parts.length !== 4) return { start: ip, end: ip };
  const [a, b, c] = parts;
  return { start: `${a}.${b}.${c}.0`, end: `${a}.${b}.${c}.255` };
}
