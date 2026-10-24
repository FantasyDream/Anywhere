import importlib.util
import json
from pathlib import Path
import subprocess
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location(
    'desktop_measurement',
    Path(__file__).resolve().parents[1] / 'measure-desktop.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class DesktopMeasurementTests(unittest.TestCase):

    def event(self, value, **extra):
        return json.dumps(
            dict(event='ui_performance',
                 operation='query',
                 elapsed_us=value,
                 native_visible=True,
                 **extra))

    def test_reports_only_numeric_aggregates_and_excludes_hidden_frames(self):
        events = [
            self.event(value, input='SECRET', path='/private/secret')
            for value in range(1, 101)
        ]
        events += [
            'not JSON',
            json.dumps(
                dict(event='ui_performance',
                     operation='query',
                     elapsed_us=1,
                     native_visible=False))
        ]
        result = module.timings(events)
        self.assertEqual(result['operations']['query'],
                         dict(count=100, p50_us=50, p95_us=95, max_us=100))
        self.assertEqual(result['hidden'], 1)
        self.assertNotIn('SECRET', json.dumps(result))
        self.assertNotIn('/private', json.dumps(result))

    def test_invalid_values_and_overflow_are_counted_without_unbounded_retention(
            self):
        events = [self.event(True), self.event(-1), self.event('2')]
        events += [self.event(value) for value in range(1, 5)]
        result = module.timings(events, limit=2)
        self.assertEqual(result['rejected'], 3)
        self.assertEqual(result['overflow'], 2)
        self.assertEqual(result['operations']['query']['count'], 2)

    def test_clipboard_and_shelf_samples_remain_separate(self):
        events = [
            json.dumps(
                dict(event='ui_performance',
                     operation=operation,
                     elapsed_us=value,
                     native_visible=True))
            for operation, value in [('clipboard_query',
                                      120000), ('shelf_query', 125000)]
        ]
        result = module.timings(events)
        self.assertEqual(result['operations']['clipboard_query']['p95_us'],
                         120000)
        self.assertEqual(result['operations']['shelf_query']['p95_us'], 125000)

    def test_footprint_timeout_and_unavailable_tool_leave_measurement_missing(
            self):
        for error in [
                subprocess.TimeoutExpired('vmmap', 15),
                FileNotFoundError('vmmap'),
        ]:
            with self.subTest(error=type(error).__name__), mock.patch.object(
                    module.subprocess, 'run', side_effect=error) as run:
                self.assertIsNone(module.footprint(123))
                self.assertEqual(run.call_args.kwargs['timeout'],
                                 module.FOOTPRINT_TIMEOUT_SECONDS)

    def test_footprint_preserves_units_and_failed_samples_are_not_zero(self):
        for output, code, expected in [
            ('Physical footprint: 12.5M\n', 0, 13107200),
            ('Physical footprint: 1024K\n', 0, 1048576),
            ('Physical footprint: 1G\n', 0, 1073741824),
            ('task inspection denied', 1, None),
            ('no footprint field', 0, None),
        ]:
            with self.subTest(output=output), mock.patch.object(
                    module.subprocess,
                    'run',
                    return_value=subprocess.CompletedProcess([], code, output)):
                self.assertEqual(module.footprint(123), expected)


if __name__ == '__main__':
    unittest.main()
