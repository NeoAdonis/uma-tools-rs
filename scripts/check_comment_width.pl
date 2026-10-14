#!/usr/bin/env perl

use v5.36;
use utf8;
use open qw(:std :encoding(UTF-8));

use File::Find;

# Checks that comment lines in Rust files are at most this many characters
# long, sigils included and indentation excluded. Only comments on their own
# line are checked.
my $max_width = 80;

my @paths = @ARGV ? @ARGV : qw(src tests);

my @files;
find(
    sub { push @files, $File::Find::name if -f && /\.rs\z/ },
    grep { -e or die "Cannot find $_\n" } @paths
);

my $errors = 0;

for my $file ( sort @files ) {
    open my $fh, '<', $file or die "Cannot open $file: $!";
    while ( my $line = <$fh> ) {
        chomp $line;
        next unless $line =~ m{^\s*(//.*)$};
        my $width = length $1;
        next if $width <= $max_width;
        say "$file:$.: comment is $width characters long (max $max_width)";
        $errors++;
    }
    close $fh;
}

if ($errors) {
    say STDERR "Found $errors comment line(s) over $max_width characters.";
    exit 1;
}
