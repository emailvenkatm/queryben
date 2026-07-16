export function sqlLiteral(value: unknown): string {
  if (value === null || value === undefined) return 'NULL';
  if (typeof value === 'boolean') return value ? '1' : '0';
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return 'NULL';
    return String(value);
  }
  if (typeof value === 'bigint') return value.toString();
  if (value instanceof Date) return `'${value.toISOString()}'`;
  if (value instanceof Uint8Array) {
    let hex = '0x';
    for (const byte of value) hex += byte.toString(16).padStart(2, '0');
    return hex;
  }
  if (typeof value === 'string') return `'${value.replace(/'/g, "''")}'`;
  try { return `'${JSON.stringify(value).replace(/'/g, "''")}'`; } catch { return 'NULL'; }
}

export function bracket(ident: string): string {
  const escape = (p: string): string => `[${p.replace(/\]/g, ']]')}]`;
  const dot = ident.indexOf('.');
  if (dot === -1) return escape(ident);
  return `${escape(ident.slice(0, dot))}.${escape(ident.slice(dot + 1))}`;
}

export function qualifiedName(schema: string, name: string): string {
  return `${bracket(schema)}.${bracket(name)}`;
}
