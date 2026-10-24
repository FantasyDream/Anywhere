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

    def domain(self, rows='', creator='Anywhere[123]', identity=12):
        return (f'pid/123 = {{\n\tcreator = {creator}\n'
                f'\tuniqueid = {identity}\n'
                '\tenvironment = {\n\t\tSECRET_TOKEN = private\n\t}\n'
                f'\tservices = {{\n{rows}\n\t}}\n}}')

    def completed(self, output, code=0):
        return subprocess.CompletedProcess([], code, output)

    @mock.patch.object(module.platform, 'system', return_value='Darwin')
    def test_webkit_verifies_process_domain_and_instance_job(self, _):
        service = 'com.apple.WebKit.GPU.12345678-1234-1234-1234-123456789ABC'
        domain = self.domain(f'\t\t321 - {service}\n'
                             '\t\t0 - com.apple.WebKit.GPU\n'
                             '\t\t900 - com.apple.OtherService')
        with mock.patch.object(
                module.subprocess,
                'run',
                side_effect=[
                    self.completed(domain),
                    self.completed('\tstate = running\n\tpid = 321\n')
                ]) as run:
            result = module.webkit_helpers(123)
            self.assertEqual(result,
                             [dict(pid=321, name='com.apple.WebKit.GPU')])
            self.assertEqual(run.call_args_list[1].args[0],
                             ['/bin/launchctl', 'print', f'pid/123/{service}'])
            for call in run.call_args_list:
                self.assertEqual(call.kwargs['timeout'],
                                 module.WEBKIT_INSPECTION_TIMEOUT_SECONDS)
            self.assertNotIn('SECRET_TOKEN', json.dumps(result))
            self.assertNotIn('OtherService', json.dumps(result))

    @mock.patch.object(module.platform, 'system', return_value='Darwin')
    def test_webkit_missing_or_foreign_domain_is_unknown_not_empty(self, _):
        for output, code in [
            ('permission denied', 1),
            (self.domain(creator='launchctl[987]'), 0),
            (self.domain(identity=0), 0),
            (self.domain().replace('pid/123', 'pid/456'), 0),
        ]:
            with self.subTest(output=output), mock.patch.object(
                    module.subprocess,
                    'run',
                    return_value=self.completed(output, code)):
                self.assertIsNone(module.webkit_helpers(123))
        with mock.patch.object(module.subprocess,
                               'run',
                               return_value=self.completed(self.domain())):
            self.assertEqual(module.webkit_helpers(123), [])

    @mock.patch.object(module.platform, 'system', return_value='Darwin')
    def test_webkit_refuses_stale_job_or_unbounded_inspection(self, _):
        service = 'com.apple.WebKit.GPU.12345678-1234-1234-1234-123456789ABC'
        domain = self.domain(f'\t\t321 - {service}')
        for job in [
                '\tstate = running\n\tpid = 999\n',
                '\tstate = not running\n\tpid = 321\n'
        ]:
            with self.subTest(job=job), mock.patch.object(
                    module.subprocess,
                    'run',
                    side_effect=[self.completed(domain),
                                 self.completed(job)]):
                self.assertIsNone(module.webkit_helpers(123))
        rows = '\n'.join(f'\t\t{pid} - {service}'
                         for pid in range(321, 322 + module.MAX_WEBKIT_HELPERS))
        with mock.patch.object(module.subprocess,
                               'run',
                               return_value=self.completed(
                                   self.domain(rows))) as run:
            self.assertIsNone(module.webkit_helpers(123))
            self.assertEqual(run.call_count, 1)

    @mock.patch.object(module.platform, 'system', return_value='Darwin')
    def test_webkit_timeout_and_missing_command_do_not_report_zero(self, _):
        for error in [
                FileNotFoundError('launchctl'),
                subprocess.TimeoutExpired('launchctl', 2)
        ]:
            with self.subTest(error=type(error).__name__), mock.patch.object(
                    module.subprocess, 'run', side_effect=error):
                self.assertIsNone(module.webkit_helpers(123))

    @mock.patch.object(module.platform, 'system', return_value='Darwin')
    def test_changed_active_service_format_is_unknown_not_empty(self, _):
        for row in [
                '321 - com.apple.WebKit.GPU', '321 - com.apple.WebKit.GPU  ',
                '321 - com.apple.WebKit.GPU.not-a-uuid',
                'running com.apple.WebKit.Networking'
        ]:
            with self.subTest(row=row), mock.patch.object(
                    module.subprocess,
                    'run',
                    return_value=self.completed(self.domain(row))):
                self.assertIsNone(module.webkit_helpers(123))

    def test_webkit_is_opt_in_and_unavailable_off_macos(self):
        with mock.patch.object(module.platform, 'system', return_value='Linux'), \
                mock.patch.object(module.subprocess, 'run') as run:
            self.assertIsNone(module.webkit_helpers(123))
            run.assert_not_called()
        with mock.patch.object(module.subprocess, 'check_output',
                               return_value='123 1 100 Anywhere\n'), \
                mock.patch.object(module, 'webkit_helpers') as inspect:
            self.assertNotIn('webkit_helpers', module.process_sample(123))
            inspect.assert_not_called()

    def test_verified_helpers_are_separate_from_descendants_and_unrelated_webkit(
            self):
        processes = ('123 1 100 Anywhere\n'
                     '321 1 20 /System/WebKit/com.apple.WebKit.GPU\n'
                     '900 1 999 /System/WebKit/com.apple.WebKit.WebContent\n')
        with mock.patch.object(module.subprocess, 'check_output',
                               return_value=processes), \
                mock.patch.object(module, 'webkit_helpers',
                                  return_value=[dict(pid=321, name='com.apple.WebKit.GPU')]):
            sample = module.process_sample(123, include_webkit=True)
        self.assertEqual(sample['descendants'], [])
        self.assertEqual(
            sample['webkit_helpers'],
            [dict(pid=321, name='com.apple.WebKit.GPU', rss_bytes=20480)])

    def test_helper_pid_reuse_and_rss_missing_are_not_measured_as_zero(self):
        for processes, expected in [
            ('123 1 100 Anywhere\n321 1 20 unrelated\n', None),
            ('123 1 100 Anywhere\n',
             [dict(pid=321, name='com.apple.WebKit.GPU', rss_bytes=None)]),
        ]:
            with self.subTest(processes=processes), mock.patch.object(
                    module.subprocess, 'check_output', return_value=processes), \
                    mock.patch.object(module, 'webkit_helpers',
                                      return_value=[dict(pid=321, name='com.apple.WebKit.GPU')]):
                self.assertEqual(
                    module.process_sample(
                        123, include_webkit=True)['webkit_helpers'], expected)

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
