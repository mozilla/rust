// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-tidy-linelength

// DO NOT EDIT. Automatically generated by 'src/etc/regex-match-tests'
// on 2014-04-23 01:33:36.539280.

// Tests from basic.dat
mat!{match_basic_3, r"abracadabra$", r"abracadabracadabra", Some((7, 18))}
mat!{match_basic_4, r"a...b", r"abababbb", Some((2, 7))}
mat!{match_basic_5, r"XXXXXX", r"..XXXXXX", Some((2, 8))}
mat!{match_basic_6, r"\)", r"()", Some((1, 2))}
mat!{match_basic_7, r"a]", r"a]a", Some((0, 2))}
mat!{match_basic_9, r"\}", r"}", Some((0, 1))}
mat!{match_basic_10, r"\]", r"]", Some((0, 1))}
mat!{match_basic_12, r"]", r"]", Some((0, 1))}
mat!{match_basic_15, r"^a", r"ax", Some((0, 1))}
mat!{match_basic_16, r"\^a", r"a^a", Some((1, 3))}
mat!{match_basic_17, r"a\^", r"a^", Some((0, 2))}
mat!{match_basic_18, r"a$", r"aa", Some((1, 2))}
mat!{match_basic_19, r"a\$", r"a$", Some((0, 2))}
mat!{match_basic_20, r"^$", r"", Some((0, 0))}
mat!{match_basic_21, r"$^", r"", Some((0, 0))}
mat!{match_basic_22, r"a($)", r"aa", Some((1, 2)), Some((2, 2))}
mat!{match_basic_23, r"a*(^a)", r"aa", Some((0, 1)), Some((0, 1))}
mat!{match_basic_24, r"(..)*(...)*", r"a", Some((0, 0))}
mat!{match_basic_25, r"(..)*(...)*", r"abcd", Some((0, 4)), Some((2, 4))}
mat!{match_basic_26, r"(ab|a)(bc|c)", r"abc", Some((0, 3)), Some((0, 2)), Some((2, 3))}
mat!{match_basic_27, r"(ab)c|abc", r"abc", Some((0, 3)), Some((0, 2))}
mat!{match_basic_28, r"a{0}b", r"ab", Some((1, 2))}
mat!{match_basic_29, r"(a*)(b?)(b+)b{3}", r"aaabbbbbbb", Some((0, 10)), Some((0, 3)), Some((3, 4)), Some((4, 7))}
mat!{match_basic_30, r"(a*)(b{0,1})(b{1,})b{3}", r"aaabbbbbbb", Some((0, 10)), Some((0, 3)), Some((3, 4)), Some((4, 7))}
mat!{match_basic_32, r"((a|a)|a)", r"a", Some((0, 1)), Some((0, 1)), Some((0, 1))}
mat!{match_basic_33, r"(a*)(a|aa)", r"aaaa", Some((0, 4)), Some((0, 3)), Some((3, 4))}
mat!{match_basic_34, r"a*(a.|aa)", r"aaaa", Some((0, 4)), Some((2, 4))}
mat!{match_basic_35, r"a(b)|c(d)|a(e)f", r"aef", Some((0, 3)), None, None, Some((1, 2))}
mat!{match_basic_36, r"(a|b)?.*", r"b", Some((0, 1)), Some((0, 1))}
mat!{match_basic_37, r"(a|b)c|a(b|c)", r"ac", Some((0, 2)), Some((0, 1))}
mat!{match_basic_38, r"(a|b)c|a(b|c)", r"ab", Some((0, 2)), None, Some((1, 2))}
mat!{match_basic_39, r"(a|b)*c|(a|ab)*c", r"abc", Some((0, 3)), Some((1, 2))}
mat!{match_basic_40, r"(a|b)*c|(a|ab)*c", r"xc", Some((1, 2))}
mat!{match_basic_41, r"(.a|.b).*|.*(.a|.b)", r"xa", Some((0, 2)), Some((0, 2))}
mat!{match_basic_42, r"a?(ab|ba)ab", r"abab", Some((0, 4)), Some((0, 2))}
mat!{match_basic_43, r"a?(ac{0}b|ba)ab", r"abab", Some((0, 4)), Some((0, 2))}
mat!{match_basic_44, r"ab|abab", r"abbabab", Some((0, 2))}
mat!{match_basic_45, r"aba|bab|bba", r"baaabbbaba", Some((5, 8))}
mat!{match_basic_46, r"aba|bab", r"baaabbbaba", Some((6, 9))}
mat!{match_basic_47, r"(aa|aaa)*|(a|aaaaa)", r"aa", Some((0, 2)), Some((0, 2))}
mat!{match_basic_48, r"(a.|.a.)*|(a|.a...)", r"aa", Some((0, 2)), Some((0, 2))}
mat!{match_basic_49, r"ab|a", r"xabc", Some((1, 3))}
mat!{match_basic_50, r"ab|a", r"xxabc", Some((2, 4))}
mat!{match_basic_51, r"(?i)(Ab|cD)*", r"aBcD", Some((0, 4)), Some((2, 4))}
mat!{match_basic_52, r"[^-]", r"--a", Some((2, 3))}
mat!{match_basic_53, r"[a-]*", r"--a", Some((0, 3))}
mat!{match_basic_54, r"[a-m-]*", r"--amoma--", Some((0, 4))}
mat!{match_basic_55, r":::1:::0:|:::1:1:0:", r":::0:::1:::1:::0:", Some((8, 17))}
mat!{match_basic_56, r":::1:::0:|:::1:1:1:", r":::0:::1:::1:::0:", Some((8, 17))}
mat!{match_basic_57, r"[[:upper:]]", r"A", Some((0, 1))}
mat!{match_basic_58, r"[[:lower:]]+", r"`az{", Some((1, 3))}
mat!{match_basic_59, r"[[:upper:]]+", r"@AZ[", Some((1, 3))}
mat!{match_basic_65, r"
", r"
", Some((0, 1))}
mat!{match_basic_66, r"
", r"
", Some((0, 1))}
mat!{match_basic_67, r"[^a]", r"
", Some((0, 1))}
mat!{match_basic_68, r"
a", r"
a", Some((0, 2))}
mat!{match_basic_69, r"(a)(b)(c)", r"abc", Some((0, 3)), Some((0, 1)), Some((1, 2)), Some((2, 3))}
mat!{match_basic_70, r"xxx", r"xxx", Some((0, 3))}
mat!{match_basic_71, r"(^|[ (,;])((([Ff]eb[^ ]* *|0*2/|\* */?)0*[6-7]))([^0-9]|$)", r"feb 6,", Some((0, 6))}
mat!{match_basic_72, r"(^|[ (,;])((([Ff]eb[^ ]* *|0*2/|\* */?)0*[6-7]))([^0-9]|$)", r"2/7", Some((0, 3))}
mat!{match_basic_73, r"(^|[ (,;])((([Ff]eb[^ ]* *|0*2/|\* */?)0*[6-7]))([^0-9]|$)", r"feb 1,Feb 6", Some((5, 11))}
mat!{match_basic_74, r"((((((((((((((((((((((((((((((x))))))))))))))))))))))))))))))", r"x", Some((0, 1)), Some((0, 1)), Some((0, 1))}
mat!{match_basic_75, r"((((((((((((((((((((((((((((((x))))))))))))))))))))))))))))))*", r"xx", Some((0, 2)), Some((1, 2)), Some((1, 2))}
mat!{match_basic_76, r"a?(ab|ba)*", r"ababababababababababababababababababababababababababababababababababababababababa", Some((0, 81)), Some((79, 81))}
mat!{match_basic_77, r"abaa|abbaa|abbbaa|abbbbaa", r"ababbabbbabbbabbbbabbbbaa", Some((18, 25))}
mat!{match_basic_78, r"abaa|abbaa|abbbaa|abbbbaa", r"ababbabbbabbbabbbbabaa", Some((18, 22))}
mat!{match_basic_79, r"aaac|aabc|abac|abbc|baac|babc|bbac|bbbc", r"baaabbbabac", Some((7, 11))}
mat!{match_basic_80, r".*", r"", Some((0, 2))}
mat!{match_basic_81, r"aaaa|bbbb|cccc|ddddd|eeeeee|fffffff|gggg|hhhh|iiiii|jjjjj|kkkkk|llll", r"XaaaXbbbXcccXdddXeeeXfffXgggXhhhXiiiXjjjXkkkXlllXcbaXaaaa", Some((53, 57))}
mat!{match_basic_83, r"a*a*a*a*a*b", r"aaaaaaaaab", Some((0, 10))}
mat!{match_basic_84, r"^", r"", Some((0, 0))}
mat!{match_basic_85, r"$", r"", Some((0, 0))}
mat!{match_basic_86, r"^$", r"", Some((0, 0))}
mat!{match_basic_87, r"^a$", r"a", Some((0, 1))}
mat!{match_basic_88, r"abc", r"abc", Some((0, 3))}
mat!{match_basic_89, r"abc", r"xabcy", Some((1, 4))}
mat!{match_basic_90, r"abc", r"ababc", Some((2, 5))}
mat!{match_basic_91, r"ab*c", r"abc", Some((0, 3))}
mat!{match_basic_92, r"ab*bc", r"abc", Some((0, 3))}
mat!{match_basic_93, r"ab*bc", r"abbc", Some((0, 4))}
mat!{match_basic_94, r"ab*bc", r"abbbbc", Some((0, 6))}
mat!{match_basic_95, r"ab+bc", r"abbc", Some((0, 4))}
mat!{match_basic_96, r"ab+bc", r"abbbbc", Some((0, 6))}
mat!{match_basic_97, r"ab?bc", r"abbc", Some((0, 4))}
mat!{match_basic_98, r"ab?bc", r"abc", Some((0, 3))}
mat!{match_basic_99, r"ab?c", r"abc", Some((0, 3))}
mat!{match_basic_100, r"^abc$", r"abc", Some((0, 3))}
mat!{match_basic_101, r"^abc", r"abcc", Some((0, 3))}
mat!{match_basic_102, r"abc$", r"aabc", Some((1, 4))}
mat!{match_basic_103, r"^", r"abc", Some((0, 0))}
mat!{match_basic_104, r"$", r"abc", Some((3, 3))}
mat!{match_basic_105, r"a.c", r"abc", Some((0, 3))}
mat!{match_basic_106, r"a.c", r"axc", Some((0, 3))}
mat!{match_basic_107, r"a.*c", r"axyzc", Some((0, 5))}
mat!{match_basic_108, r"a[bc]d", r"abd", Some((0, 3))}
mat!{match_basic_109, r"a[b-d]e", r"ace", Some((0, 3))}
mat!{match_basic_110, r"a[b-d]", r"aac", Some((1, 3))}
mat!{match_basic_111, r"a[-b]", r"a-", Some((0, 2))}
mat!{match_basic_112, r"a[b-]", r"a-", Some((0, 2))}
mat!{match_basic_113, r"a]", r"a]", Some((0, 2))}
mat!{match_basic_114, r"a.index(&FullRange)]b", r"a]b", Some((0, 3))}
mat!{match_basic_115, r"a[^bc]d", r"aed", Some((0, 3))}
mat!{match_basic_116, r"a[^-b]c", r"adc", Some((0, 3))}
mat!{match_basic_117, r"a[^]b]c", r"adc", Some((0, 3))}
mat!{match_basic_118, r"ab|cd", r"abc", Some((0, 2))}
mat!{match_basic_119, r"ab|cd", r"abcd", Some((0, 2))}
mat!{match_basic_120, r"a\(b", r"a(b", Some((0, 3))}
mat!{match_basic_121, r"a\(*b", r"ab", Some((0, 2))}
mat!{match_basic_122, r"a\(*b", r"a((b", Some((0, 4))}
mat!{match_basic_123, r"((a))", r"abc", Some((0, 1)), Some((0, 1)), Some((0, 1))}
mat!{match_basic_124, r"(a)b(c)", r"abc", Some((0, 3)), Some((0, 1)), Some((2, 3))}
mat!{match_basic_125, r"a+b+c", r"aabbabc", Some((4, 7))}
mat!{match_basic_126, r"a*", r"aaa", Some((0, 3))}
mat!{match_basic_128, r"(a*)*", r"-", Some((0, 0)), None}
mat!{match_basic_129, r"(a*)+", r"-", Some((0, 0)), Some((0, 0))}
mat!{match_basic_131, r"(a*|b)*", r"-", Some((0, 0)), None}
mat!{match_basic_132, r"(a+|b)*", r"ab", Some((0, 2)), Some((1, 2))}
mat!{match_basic_133, r"(a+|b)+", r"ab", Some((0, 2)), Some((1, 2))}
mat!{match_basic_134, r"(a+|b)?", r"ab", Some((0, 1)), Some((0, 1))}
mat!{match_basic_135, r"[^ab]*", r"cde", Some((0, 3))}
mat!{match_basic_137, r"(^)*", r"-", Some((0, 0)), None}
mat!{match_basic_138, r"a*", r"", Some((0, 0))}
mat!{match_basic_139, r"([abc])*d", r"abbbcd", Some((0, 6)), Some((4, 5))}
mat!{match_basic_140, r"([abc])*bcd", r"abcd", Some((0, 4)), Some((0, 1))}
mat!{match_basic_141, r"a|b|c|d|e", r"e", Some((0, 1))}
mat!{match_basic_142, r"(a|b|c|d|e)f", r"ef", Some((0, 2)), Some((0, 1))}
mat!{match_basic_144, r"((a*|b))*", r"-", Some((0, 0)), None, None}
mat!{match_basic_145, r"abcd*efg", r"abcdefg", Some((0, 7))}
mat!{match_basic_146, r"ab*", r"xabyabbbz", Some((1, 3))}
mat!{match_basic_147, r"ab*", r"xayabbbz", Some((1, 2))}
mat!{match_basic_148, r"(ab|cd)e", r"abcde", Some((2, 5)), Some((2, 4))}
mat!{match_basic_149, r"[abhgefdc]ij", r"hij", Some((0, 3))}
mat!{match_basic_150, r"(a|b)c*d", r"abcd", Some((1, 4)), Some((1, 2))}
mat!{match_basic_151, r"(ab|ab*)bc", r"abc", Some((0, 3)), Some((0, 1))}
mat!{match_basic_152, r"a([bc]*)c*", r"abc", Some((0, 3)), Some((1, 3))}
mat!{match_basic_153, r"a([bc]*)(c*d)", r"abcd", Some((0, 4)), Some((1, 3)), Some((3, 4))}
mat!{match_basic_154, r"a([bc]+)(c*d)", r"abcd", Some((0, 4)), Some((1, 3)), Some((3, 4))}
mat!{match_basic_155, r"a([bc]*)(c+d)", r"abcd", Some((0, 4)), Some((1, 2)), Some((2, 4))}
mat!{match_basic_156, r"a[bcd]*dcdcde", r"adcdcde", Some((0, 7))}
mat!{match_basic_157, r"(ab|a)b*c", r"abc", Some((0, 3)), Some((0, 2))}
mat!{match_basic_158, r"((a)(b)c)(d)", r"abcd", Some((0, 4)), Some((0, 3)), Some((0, 1)), Some((1, 2)), Some((3, 4))}
mat!{match_basic_159, r"[A-Za-z_][A-Za-z0-9_]*", r"alpha", Some((0, 5))}
mat!{match_basic_160, r"^a(bc+|b[eh])g|.h$", r"abh", Some((1, 3))}
mat!{match_basic_161, r"(bc+d$|ef*g.|h?i(j|k))", r"effgz", Some((0, 5)), Some((0, 5))}
mat!{match_basic_162, r"(bc+d$|ef*g.|h?i(j|k))", r"ij", Some((0, 2)), Some((0, 2)), Some((1, 2))}
mat!{match_basic_163, r"(bc+d$|ef*g.|h?i(j|k))", r"reffgz", Some((1, 6)), Some((1, 6))}
mat!{match_basic_164, r"(((((((((a)))))))))", r"a", Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1)), Some((0, 1))}
mat!{match_basic_165, r"multiple words", r"multiple words yeah", Some((0, 14))}
mat!{match_basic_166, r"(.*)c(.*)", r"abcde", Some((0, 5)), Some((0, 2)), Some((3, 5))}
mat!{match_basic_167, r"abcd", r"abcd", Some((0, 4))}
mat!{match_basic_168, r"a(bc)d", r"abcd", Some((0, 4)), Some((1, 3))}
mat!{match_basic_169, r"a[-]?c", r"ac", Some((0, 3))}
mat!{match_basic_170, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Qaddafi", Some((0, 15)), None, Some((10, 12))}
mat!{match_basic_171, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Mo'ammar Gadhafi", Some((0, 16)), None, Some((11, 13))}
mat!{match_basic_172, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Kaddafi", Some((0, 15)), None, Some((10, 12))}
mat!{match_basic_173, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Qadhafi", Some((0, 15)), None, Some((10, 12))}
mat!{match_basic_174, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Gadafi", Some((0, 14)), None, Some((10, 11))}
mat!{match_basic_175, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Mu'ammar Qadafi", Some((0, 15)), None, Some((11, 12))}
mat!{match_basic_176, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Moamar Gaddafi", Some((0, 14)), None, Some((9, 11))}
mat!{match_basic_177, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Mu'ammar Qadhdhafi", Some((0, 18)), None, Some((13, 15))}
mat!{match_basic_178, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Khaddafi", Some((0, 16)), None, Some((11, 13))}
mat!{match_basic_179, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Ghaddafy", Some((0, 16)), None, Some((11, 13))}
mat!{match_basic_180, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Ghadafi", Some((0, 15)), None, Some((11, 12))}
mat!{match_basic_181, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Ghaddafi", Some((0, 16)), None, Some((11, 13))}
mat!{match_basic_182, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muamar Kaddafi", Some((0, 14)), None, Some((9, 11))}
mat!{match_basic_183, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Quathafi", Some((0, 16)), None, Some((11, 13))}
mat!{match_basic_184, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Muammar Gheddafi", Some((0, 16)), None, Some((11, 13))}
mat!{match_basic_185, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Moammar Khadafy", Some((0, 15)), None, Some((11, 12))}
mat!{match_basic_186, r"M[ou]'?am+[ae]r .*([AEae]l[- ])?[GKQ]h?[aeu]+([dtz][dhz]?)+af[iy]", r"Moammar Qudhafi", Some((0, 15)), None, Some((10, 12))}
mat!{match_basic_187, r"a+(b|c)*d+", r"aabcdd", Some((0, 6)), Some((3, 4))}
mat!{match_basic_188, r"^.+$", r"vivi", Some((0, 4))}
mat!{match_basic_189, r"^(.+)$", r"vivi", Some((0, 4)), Some((0, 4))}
mat!{match_basic_190, r"^([^!.]+).att.com!(.+)$", r"gryphon.att.com!eby", Some((0, 19)), Some((0, 7)), Some((16, 19))}
mat!{match_basic_191, r"^([^!]+!)?([^!]+)$", r"bas", Some((0, 3)), None, Some((0, 3))}
mat!{match_basic_192, r"^([^!]+!)?([^!]+)$", r"bar!bas", Some((0, 7)), Some((0, 4)), Some((4, 7))}
mat!{match_basic_193, r"^([^!]+!)?([^!]+)$", r"foo!bas", Some((0, 7)), Some((0, 4)), Some((4, 7))}
mat!{match_basic_194, r"^.+!([^!]+!)([^!]+)$", r"foo!bar!bas", Some((0, 11)), Some((4, 8)), Some((8, 11))}
mat!{match_basic_195, r"((foo)|(bar))!bas", r"bar!bas", Some((0, 7)), Some((0, 3)), None, Some((0, 3))}
mat!{match_basic_196, r"((foo)|(bar))!bas", r"foo!bar!bas", Some((4, 11)), Some((4, 7)), None, Some((4, 7))}
mat!{match_basic_197, r"((foo)|(bar))!bas", r"foo!bas", Some((0, 7)), Some((0, 3)), Some((0, 3))}
mat!{match_basic_198, r"((foo)|bar)!bas", r"bar!bas", Some((0, 7)), Some((0, 3))}
mat!{match_basic_199, r"((foo)|bar)!bas", r"foo!bar!bas", Some((4, 11)), Some((4, 7))}
mat!{match_basic_200, r"((foo)|bar)!bas", r"foo!bas", Some((0, 7)), Some((0, 3)), Some((0, 3))}
mat!{match_basic_201, r"(foo|(bar))!bas", r"bar!bas", Some((0, 7)), Some((0, 3)), Some((0, 3))}
mat!{match_basic_202, r"(foo|(bar))!bas", r"foo!bar!bas", Some((4, 11)), Some((4, 7)), Some((4, 7))}
mat!{match_basic_203, r"(foo|(bar))!bas", r"foo!bas", Some((0, 7)), Some((0, 3))}
mat!{match_basic_204, r"(foo|bar)!bas", r"bar!bas", Some((0, 7)), Some((0, 3))}
mat!{match_basic_205, r"(foo|bar)!bas", r"foo!bar!bas", Some((4, 11)), Some((4, 7))}
mat!{match_basic_206, r"(foo|bar)!bas", r"foo!bas", Some((0, 7)), Some((0, 3))}
mat!{match_basic_207, r"^(([^!]+!)?([^!]+)|.+!([^!]+!)([^!]+))$", r"foo!bar!bas", Some((0, 11)), Some((0, 11)), None, None, Some((4, 8)), Some((8, 11))}
mat!{match_basic_208, r"^([^!]+!)?([^!]+)$|^.+!([^!]+!)([^!]+)$", r"bas", Some((0, 3)), None, Some((0, 3))}
mat!{match_basic_209, r"^([^!]+!)?([^!]+)$|^.+!([^!]+!)([^!]+)$", r"bar!bas", Some((0, 7)), Some((0, 4)), Some((4, 7))}
mat!{match_basic_210, r"^([^!]+!)?([^!]+)$|^.+!([^!]+!)([^!]+)$", r"foo!bar!bas", Some((0, 11)), None, None, Some((4, 8)), Some((8, 11))}
mat!{match_basic_211, r"^([^!]+!)?([^!]+)$|^.+!([^!]+!)([^!]+)$", r"foo!bas", Some((0, 7)), Some((0, 4)), Some((4, 7))}
mat!{match_basic_212, r"^(([^!]+!)?([^!]+)|.+!([^!]+!)([^!]+))$", r"bas", Some((0, 3)), Some((0, 3)), None, Some((0, 3))}
mat!{match_basic_213, r"^(([^!]+!)?([^!]+)|.+!([^!]+!)([^!]+))$", r"bar!bas", Some((0, 7)), Some((0, 7)), Some((0, 4)), Some((4, 7))}
mat!{match_basic_214, r"^(([^!]+!)?([^!]+)|.+!([^!]+!)([^!]+))$", r"foo!bar!bas", Some((0, 11)), Some((0, 11)), None, None, Some((4, 8)), Some((8, 11))}
mat!{match_basic_215, r"^(([^!]+!)?([^!]+)|.+!([^!]+!)([^!]+))$", r"foo!bas", Some((0, 7)), Some((0, 7)), Some((0, 4)), Some((4, 7))}
mat!{match_basic_216, r".*(/XXX).*", r"/XXX", Some((0, 4)), Some((0, 4))}
mat!{match_basic_217, r".*(\\XXX).*", r"\XXX", Some((0, 4)), Some((0, 4))}
mat!{match_basic_218, r"\\XXX", r"\XXX", Some((0, 4))}
mat!{match_basic_219, r".*(/000).*", r"/000", Some((0, 4)), Some((0, 4))}
mat!{match_basic_220, r".*(\\000).*", r"\000", Some((0, 4)), Some((0, 4))}
mat!{match_basic_221, r"\\000", r"\000", Some((0, 4))}

// Tests from nullsubexpr.dat
mat!{match_nullsubexpr_3, r"(a*)*", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_5, r"(a*)*", r"x", Some((0, 0)), None}
mat!{match_nullsubexpr_6, r"(a*)*", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_7, r"(a*)*", r"aaaaaax", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_8, r"(a*)+", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_9, r"(a*)+", r"x", Some((0, 0)), Some((0, 0))}
mat!{match_nullsubexpr_10, r"(a*)+", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_11, r"(a*)+", r"aaaaaax", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_12, r"(a+)*", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_13, r"(a+)*", r"x", Some((0, 0))}
mat!{match_nullsubexpr_14, r"(a+)*", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_15, r"(a+)*", r"aaaaaax", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_16, r"(a+)+", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_17, r"(a+)+", r"x", None}
mat!{match_nullsubexpr_18, r"(a+)+", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_19, r"(a+)+", r"aaaaaax", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_21, r"([a]*)*", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_23, r"([a]*)*", r"x", Some((0, 0)), None}
mat!{match_nullsubexpr_24, r"([a]*)*", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_25, r"([a]*)*", r"aaaaaax", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_26, r"([a]*)+", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_27, r"([a]*)+", r"x", Some((0, 0)), Some((0, 0))}
mat!{match_nullsubexpr_28, r"([a]*)+", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_29, r"([a]*)+", r"aaaaaax", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_30, r"([^b]*)*", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_32, r"([^b]*)*", r"b", Some((0, 0)), None}
mat!{match_nullsubexpr_33, r"([^b]*)*", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_34, r"([^b]*)*", r"aaaaaab", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_35, r"([ab]*)*", r"a", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_36, r"([ab]*)*", r"aaaaaa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_37, r"([ab]*)*", r"ababab", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_38, r"([ab]*)*", r"bababa", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_39, r"([ab]*)*", r"b", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_40, r"([ab]*)*", r"bbbbbb", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_41, r"([ab]*)*", r"aaaabcde", Some((0, 5)), Some((0, 5))}
mat!{match_nullsubexpr_42, r"([^a]*)*", r"b", Some((0, 1)), Some((0, 1))}
mat!{match_nullsubexpr_43, r"([^a]*)*", r"bbbbbb", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_45, r"([^a]*)*", r"aaaaaa", Some((0, 0)), None}
mat!{match_nullsubexpr_46, r"([^ab]*)*", r"ccccxx", Some((0, 6)), Some((0, 6))}
mat!{match_nullsubexpr_48, r"([^ab]*)*", r"ababab", Some((0, 0)), None}
mat!{match_nullsubexpr_50, r"((z)+|a)*", r"zabcde", Some((0, 2)), Some((1, 2))}
mat!{match_nullsubexpr_69, r"(a*)*(x)", r"x", Some((0, 1)), None, Some((0, 1))}
mat!{match_nullsubexpr_70, r"(a*)*(x)", r"ax", Some((0, 2)), Some((0, 1)), Some((1, 2))}
mat!{match_nullsubexpr_71, r"(a*)*(x)", r"axa", Some((0, 2)), Some((0, 1)), Some((1, 2))}
mat!{match_nullsubexpr_73, r"(a*)+(x)", r"x", Some((0, 1)), Some((0, 0)), Some((0, 1))}
mat!{match_nullsubexpr_74, r"(a*)+(x)", r"ax", Some((0, 2)), Some((0, 1)), Some((1, 2))}
mat!{match_nullsubexpr_75, r"(a*)+(x)", r"axa", Some((0, 2)), Some((0, 1)), Some((1, 2))}
mat!{match_nullsubexpr_77, r"(a*){2}(x)", r"x", Some((0, 1)), Some((0, 0)), Some((0, 1))}
mat!{match_nullsubexpr_78, r"(a*){2}(x)", r"ax", Some((0, 2)), Some((1, 1)), Some((1, 2))}
mat!{match_nullsubexpr_79, r"(a*){2}(x)", r"axa", Some((0, 2)), Some((1, 1)), Some((1, 2))}

// Tests from repetition.dat
mat!{match_repetition_10, r"((..)|(.))", r"", None}
mat!{match_repetition_11, r"((..)|(.))((..)|(.))", r"", None}
mat!{match_repetition_12, r"((..)|(.))((..)|(.))((..)|(.))", r"", None}
mat!{match_repetition_14, r"((..)|(.)){1}", r"", None}
mat!{match_repetition_15, r"((..)|(.)){2}", r"", None}
mat!{match_repetition_16, r"((..)|(.)){3}", r"", None}
mat!{match_repetition_18, r"((..)|(.))*", r"", Some((0, 0))}
mat!{match_repetition_20, r"((..)|(.))", r"a", Some((0, 1)), Some((0, 1)), None, Some((0, 1))}
mat!{match_repetition_21, r"((..)|(.))((..)|(.))", r"a", None}
mat!{match_repetition_22, r"((..)|(.))((..)|(.))((..)|(.))", r"a", None}
mat!{match_repetition_24, r"((..)|(.)){1}", r"a", Some((0, 1)), Some((0, 1)), None, Some((0, 1))}
mat!{match_repetition_25, r"((..)|(.)){2}", r"a", None}
mat!{match_repetition_26, r"((..)|(.)){3}", r"a", None}
mat!{match_repetition_28, r"((..)|(.))*", r"a", Some((0, 1)), Some((0, 1)), None, Some((0, 1))}
mat!{match_repetition_30, r"((..)|(.))", r"aa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_31, r"((..)|(.))((..)|(.))", r"aa", Some((0, 2)), Some((0, 1)), None, Some((0, 1)), Some((1, 2)), None, Some((1, 2))}
mat!{match_repetition_32, r"((..)|(.))((..)|(.))((..)|(.))", r"aa", None}
mat!{match_repetition_34, r"((..)|(.)){1}", r"aa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_35, r"((..)|(.)){2}", r"aa", Some((0, 2)), Some((1, 2)), None, Some((1, 2))}
mat!{match_repetition_36, r"((..)|(.)){3}", r"aa", None}
mat!{match_repetition_38, r"((..)|(.))*", r"aa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_40, r"((..)|(.))", r"aaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_41, r"((..)|(.))((..)|(.))", r"aaa", Some((0, 3)), Some((0, 2)), Some((0, 2)), None, Some((2, 3)), None, Some((2, 3))}
mat!{match_repetition_42, r"((..)|(.))((..)|(.))((..)|(.))", r"aaa", Some((0, 3)), Some((0, 1)), None, Some((0, 1)), Some((1, 2)), None, Some((1, 2)), Some((2, 3)), None, Some((2, 3))}
mat!{match_repetition_44, r"((..)|(.)){1}", r"aaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_46, r"((..)|(.)){2}", r"aaa", Some((0, 3)), Some((2, 3)), Some((0, 2)), Some((2, 3))}
mat!{match_repetition_47, r"((..)|(.)){3}", r"aaa", Some((0, 3)), Some((2, 3)), None, Some((2, 3))}
mat!{match_repetition_50, r"((..)|(.))*", r"aaa", Some((0, 3)), Some((2, 3)), Some((0, 2)), Some((2, 3))}
mat!{match_repetition_52, r"((..)|(.))", r"aaaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_53, r"((..)|(.))((..)|(.))", r"aaaa", Some((0, 4)), Some((0, 2)), Some((0, 2)), None, Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_54, r"((..)|(.))((..)|(.))((..)|(.))", r"aaaa", Some((0, 4)), Some((0, 2)), Some((0, 2)), None, Some((2, 3)), None, Some((2, 3)), Some((3, 4)), None, Some((3, 4))}
mat!{match_repetition_56, r"((..)|(.)){1}", r"aaaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_57, r"((..)|(.)){2}", r"aaaa", Some((0, 4)), Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_59, r"((..)|(.)){3}", r"aaaa", Some((0, 4)), Some((3, 4)), Some((0, 2)), Some((3, 4))}
mat!{match_repetition_61, r"((..)|(.))*", r"aaaa", Some((0, 4)), Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_63, r"((..)|(.))", r"aaaaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_64, r"((..)|(.))((..)|(.))", r"aaaaa", Some((0, 4)), Some((0, 2)), Some((0, 2)), None, Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_65, r"((..)|(.))((..)|(.))((..)|(.))", r"aaaaa", Some((0, 5)), Some((0, 2)), Some((0, 2)), None, Some((2, 4)), Some((2, 4)), None, Some((4, 5)), None, Some((4, 5))}
mat!{match_repetition_67, r"((..)|(.)){1}", r"aaaaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_68, r"((..)|(.)){2}", r"aaaaa", Some((0, 4)), Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_70, r"((..)|(.)){3}", r"aaaaa", Some((0, 5)), Some((4, 5)), Some((2, 4)), Some((4, 5))}
mat!{match_repetition_73, r"((..)|(.))*", r"aaaaa", Some((0, 5)), Some((4, 5)), Some((2, 4)), Some((4, 5))}
mat!{match_repetition_75, r"((..)|(.))", r"aaaaaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_76, r"((..)|(.))((..)|(.))", r"aaaaaa", Some((0, 4)), Some((0, 2)), Some((0, 2)), None, Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_77, r"((..)|(.))((..)|(.))((..)|(.))", r"aaaaaa", Some((0, 6)), Some((0, 2)), Some((0, 2)), None, Some((2, 4)), Some((2, 4)), None, Some((4, 6)), Some((4, 6)), None}
mat!{match_repetition_79, r"((..)|(.)){1}", r"aaaaaa", Some((0, 2)), Some((0, 2)), Some((0, 2)), None}
mat!{match_repetition_80, r"((..)|(.)){2}", r"aaaaaa", Some((0, 4)), Some((2, 4)), Some((2, 4)), None}
mat!{match_repetition_81, r"((..)|(.)){3}", r"aaaaaa", Some((0, 6)), Some((4, 6)), Some((4, 6)), None}
mat!{match_repetition_83, r"((..)|(.))*", r"aaaaaa", Some((0, 6)), Some((4, 6)), Some((4, 6)), None}
mat!{match_repetition_90, r"X(.?){0,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_91, r"X(.?){1,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_92, r"X(.?){2,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_93, r"X(.?){3,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_94, r"X(.?){4,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_95, r"X(.?){5,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_96, r"X(.?){6,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_97, r"X(.?){7,}Y", r"X1234567Y", Some((0, 9)), Some((7, 8))}
mat!{match_repetition_98, r"X(.?){8,}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_100, r"X(.?){0,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_102, r"X(.?){1,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_104, r"X(.?){2,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_106, r"X(.?){3,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_108, r"X(.?){4,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_110, r"X(.?){5,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_112, r"X(.?){6,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_114, r"X(.?){7,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_115, r"X(.?){8,8}Y", r"X1234567Y", Some((0, 9)), Some((8, 8))}
mat!{match_repetition_126, r"(a|ab|c|bcd){0,}(d*)", r"ababcd", Some((0, 1)), Some((0, 1)), Some((1, 1))}
mat!{match_repetition_127, r"(a|ab|c|bcd){1,}(d*)", r"ababcd", Some((0, 1)), Some((0, 1)), Some((1, 1))}
mat!{match_repetition_128, r"(a|ab|c|bcd){2,}(d*)", r"ababcd", Some((0, 6)), Some((3, 6)), Some((6, 6))}
mat!{match_repetition_129, r"(a|ab|c|bcd){3,}(d*)", r"ababcd", Some((0, 6)), Some((3, 6)), Some((6, 6))}
mat!{match_repetition_130, r"(a|ab|c|bcd){4,}(d*)", r"ababcd", None}
mat!{match_repetition_131, r"(a|ab|c|bcd){0,10}(d*)", r"ababcd", Some((0, 1)), Some((0, 1)), Some((1, 1))}
mat!{match_repetition_132, r"(a|ab|c|bcd){1,10}(d*)", r"ababcd", Some((0, 1)), Some((0, 1)), Some((1, 1))}
mat!{match_repetition_133, r"(a|ab|c|bcd){2,10}(d*)", r"ababcd", Some((0, 6)), Some((3, 6)), Some((6, 6))}
mat!{match_repetition_134, r"(a|ab|c|bcd){3,10}(d*)", r"ababcd", Some((0, 6)), Some((3, 6)), Some((6, 6))}
mat!{match_repetition_135, r"(a|ab|c|bcd){4,10}(d*)", r"ababcd", None}
mat!{match_repetition_136, r"(a|ab|c|bcd)*(d*)", r"ababcd", Some((0, 1)), Some((0, 1)), Some((1, 1))}
mat!{match_repetition_137, r"(a|ab|c|bcd)+(d*)", r"ababcd", Some((0, 1)), Some((0, 1)), Some((1, 1))}
mat!{match_repetition_143, r"(ab|a|c|bcd){0,}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_145, r"(ab|a|c|bcd){1,}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_147, r"(ab|a|c|bcd){2,}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_149, r"(ab|a|c|bcd){3,}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_150, r"(ab|a|c|bcd){4,}(d*)", r"ababcd", None}
mat!{match_repetition_152, r"(ab|a|c|bcd){0,10}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_154, r"(ab|a|c|bcd){1,10}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_156, r"(ab|a|c|bcd){2,10}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_158, r"(ab|a|c|bcd){3,10}(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_159, r"(ab|a|c|bcd){4,10}(d*)", r"ababcd", None}
mat!{match_repetition_161, r"(ab|a|c|bcd)*(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}
mat!{match_repetition_163, r"(ab|a|c|bcd)+(d*)", r"ababcd", Some((0, 6)), Some((4, 5)), Some((5, 6))}

