/// Estimate passphrase entropy in bits using character-pool model.
export function estimateEntropy(passphrase: string): number {
  if (!passphrase) return 0;
  let poolSize = 0;
  if (/[a-z]/.test(passphrase)) poolSize += 26;
  if (/[A-Z]/.test(passphrase)) poolSize += 26;
  if (/[0-9]/.test(passphrase)) poolSize += 10;
  if (/[^a-zA-Z0-9]/.test(passphrase)) poolSize += 32;
  if (/[^\x00-\x7F]/.test(passphrase)) poolSize += 100;
  if (poolSize === 0) return 0;
  return passphrase.length * Math.log2(poolSize);
}
