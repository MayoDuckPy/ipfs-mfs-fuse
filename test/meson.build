unit_tests = [
  'test-mfsf-cmd-files-stat.c',
]

foreach unit_test : unit_tests
  test_name = unit_test.strip('.c')
  test = executable(
    test_name,
    sources: [unit_test],
    include_directories: [inc, '../src'],
    install : false)
  test(test_name, test)
endforeach
