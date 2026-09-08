export async function readAt(host, offset, length) {
  return new Uint8Array(await host.file.slice(offset, offset + length).arrayBuffer())
}

export async function writeAt(host, offset, bytes) {
  await host.writable.write({ type: 'write', position: offset, data: bytes })
}

export async function finish(host) {
  await host.writable.close()
}
