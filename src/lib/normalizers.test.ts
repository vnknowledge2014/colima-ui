import { describe, it, expect } from 'vitest';
import { normalizeContainer, normalizeImage, normalizeVolume, normalizeNetwork } from './normalizers';

describe('normalizeContainer', () => {
  it('normalizes docker container status correctly', () => {
    const raw = { Id: 'test-id', Names: ['/test-container'], State: 'running', Status: 'Up 5 minutes', Image: 'nginx:latest' };
    const normalized = normalizeContainer(raw);
    expect(normalized.Id).toBe('test-id');
    expect(normalized.Names).toEqual(['/test-container']);
    expect(normalized.State).toBe('running');
    expect(normalized.Status).toBe('Up 5 minutes');
    expect(normalized.Image).toBe('nginx:latest');
  });

  it('handles missing or malformed fields gracefully', () => {
    const normalized = normalizeContainer({});
    expect(normalized.Id).toBe('');
    expect(normalized.Names).toBe('');
    expect(normalized.State).toBe('');
    expect(normalized.Status).toBe('');
    expect(normalized.Image).toBe('');
  });

  it('handles snake_case field names', () => {
    const raw = { id: 'abc', names: ['/foo'], state: 'exited', status: 'Exited (0)', image: 'alpine' };
    const normalized = normalizeContainer(raw);
    expect(normalized.Id).toBe('abc');
    expect(normalized.State).toBe('exited');
  });
});

describe('normalizeImage', () => {
  it('normalizes a standard image object', () => {
    const raw = { Id: 'sha256:abc123', Repository: 'nginx', Tag: 'latest', Size: '50000000', CreatedAt: '1700000000' };
    const normalized = normalizeImage(raw);
    expect(normalized.Id).toBe('sha256:abc123');
    expect(normalized.Repository).toBe('nginx');
    expect(normalized.Tag).toBe('latest');
  });

  it('handles missing fields with empty string defaults', () => {
    const normalized = normalizeImage({});
    expect(normalized.Id).toBe('');
    expect(normalized.Repository).toBe('');
    expect(normalized.Tag).toBe('');
    expect(normalized.Size).toBe('');
    expect(normalized.CreatedAt).toBe('');
  });

  it('normalizes snake_case alternative field names', () => {
    const raw = { id: 'xyz', repository: 'ubuntu', tag: '22.04', size: '100', created_at: '1700000' };
    const normalized = normalizeImage(raw);
    expect(normalized.Id).toBe('xyz');
    expect(normalized.Repository).toBe('ubuntu');
    expect(normalized.Tag).toBe('22.04');
  });
});

describe('normalizeVolume', () => {
  it('normalizes a standard volume object', () => {
    const raw = { Name: 'my-vol', Driver: 'local', Mountpoint: '/var/lib/docker/volumes/my-vol/_data', Scope: 'local' };
    const normalized = normalizeVolume(raw);
    expect(normalized.Name).toBe('my-vol');
    expect(normalized.Driver).toBe('local');
    expect(normalized.Mountpoint).toBe('/var/lib/docker/volumes/my-vol/_data');
    expect(normalized.Scope).toBe('local');
  });

  it('handles empty input with defaults', () => {
    const normalized = normalizeVolume({});
    expect(normalized.Name).toBe('');
    expect(normalized.Driver).toBe('');
    expect(normalized.Mountpoint).toBe('');
  });

  it('handles mount_point alternative casing', () => {
    const raw = { name: 'v1', driver: 'local', mount_point: '/data', scope: 'local' };
    const normalized = normalizeVolume(raw);
    expect(normalized.Mountpoint).toBe('/data');
  });
});

describe('normalizeNetwork', () => {
  it('normalizes a standard network object', () => {
    const raw = { Id: 'net-abc', Name: 'my-net', Driver: 'bridge', Scope: 'local', Ipv6: 'false', Internal: 'false' };
    const normalized = normalizeNetwork(raw);
    expect(normalized.Id).toBe('net-abc');
    expect(normalized.Name).toBe('my-net');
    expect(normalized.Driver).toBe('bridge');
    expect(normalized.Scope).toBe('local');
  });

  it('handles empty input with defaults', () => {
    const normalized = normalizeNetwork({});
    expect(normalized.Id).toBe('');
    expect(normalized.Name).toBe('');
    expect(normalized.Driver).toBe('');
    expect(normalized.Scope).toBe('');
  });

  it('handles IPv6 alternative casing', () => {
    const raw = { id: 'x', name: 'n', driver: 'd', scope: 's', IPv6: 'true', internal: 'false' };
    const normalized = normalizeNetwork(raw);
    expect(normalized.Ipv6).toBe('true');
  });
});
