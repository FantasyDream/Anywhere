import 'package:anywhere/web_plugins/web_resource_pool.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('resource admission remains bounded across document replacement',
      () async {
    final pool = WebResourcePool(running: 1, queued: 1);
    final old = Object(), next = Object();
    expect(await pool.acquire(old), true);
    final waiting = pool.acquire(next);
    expect(await pool.acquire(next), false);
    var started = false;
    waiting.then((_) => started = true);
    await Future<void>.delayed(Duration.zero);
    expect(started, false);
    pool.release();
    expect(await waiting, true);
    pool.release();
    expect(await pool.acquire(next), true);
    pool.release();
  });
  test('disposing a document drops queued reads but retains running leases',
      () async {
    final pool = WebResourcePool(running: 1, queued: 3);
    final old = Object(), next = Object();
    expect(await pool.acquire(old), true);
    final dropped = pool.acquire(old), kept = pool.acquire(next);
    pool.cancel(old);
    expect(await dropped, false);
    var completed = false;
    kept.then((_) => completed = true);
    await Future<void>.delayed(Duration.zero);
    expect(completed, false);
    pool.release();
    expect(await kept, true);
    pool.release();
  });
}
