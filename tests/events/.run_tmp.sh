cd /mnt/c/Users/hp/Documents/naclac-fw/tests/events
for p in events events_borsh events_zc; do
  echo "== $p"
  cargo test -p $p --test event_cpi_emit_test --test event_cpi_smoke_test --test event_cpi_cu_bench -- --nocapture --test-threads=1 2>&1 | grep -E 'CU|test result|FAILED|panicked|^error'
done
