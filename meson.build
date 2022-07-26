project('ipfs-mfs-fuse', 'c',
  version : '0.1',
  default_options : ['warning_level=3'])

fuse = dependency('fuse3')
deps = [fuse]

subdir('src')
subdir('include')
exe = executable(
  'ipfs-mfs-fuse',
  sources: [main, srcs],
  dependencies: deps,
  include_directories: inc,
  install : true)

subdir('test')
