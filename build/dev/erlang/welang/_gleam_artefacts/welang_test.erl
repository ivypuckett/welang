-module(welang_test).
-compile([no_auto_import, nowarn_unused_vars, nowarn_unused_function, nowarn_nomatch, inline]).
-define(FILEPATH, "test/welang_test.gleam").
-export([main/0, hello_world_test/0]).

-file("test/welang_test.gleam", 4).
-spec main() -> nil.
main() ->
    gleeunit:main().

-file("test/welang_test.gleam", 9).
-spec hello_world_test() -> nil.
hello_world_test() ->
    _pipe = 1,
    gleeunit@should:equal(_pipe, 1).
