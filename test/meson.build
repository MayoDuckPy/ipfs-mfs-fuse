unit_test = 'unit-test.c'

unit_test_exe = executable(
  unit_test.strip('.c'),
  sources: unit_test,
  include_directories: [inc, '../src'],
  dependencies: deps,
  c_args: ['-DMFSF_TEST'],
  install : false
)

test('Parse root dir', unit_test_exe, args: 'parse_root_dir', is_parallel: false)
test('Run ls', unit_test_exe, args: 'run_ls', is_parallel: false)
