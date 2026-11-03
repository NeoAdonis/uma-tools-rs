#!/usr/bin/env perl

use v5.36;
use utf8;
use open qw(:std :encoding(UTF-8));

use DBI;
use DBD::SQLite::Constants qw(:file_open :dbd_sqlite_string_mode);
use Getopt::Long;
use JSON::PP;

my $usage = <<'USAGE';
Usage: make_names.pl [--japanese jp.mdb] [--english en.mdb] names.json

Updates the Japanese and/or English names in names.json (if it exists) from
the given master databases, overwriting existing names, and prints the
result. Names missing from the databases are kept. At least one database is
required.
USAGE

# `text_data` category of the names in each section.
my %categories = (
    characters => 4,
    skills     => 47,
    tracks     => 35,
);

my ( $jpmdb, $enmdb );
GetOptions( 'japanese=s' => \$jpmdb, 'english=s' => \$enmdb )
  && @ARGV == 1
  && ( defined $jpmdb || defined $enmdb )
  or die $usage;
my ($filename) = @ARGV;

my $json = JSON::PP->new;
$json->canonical(1);

my $names = -e $filename
  ? do {
    open my $fh, '<', $filename or die "Cannot open $filename: $!";
    local $/;
    my $data = $json->decode(<$fh>);
    close $fh;
    $data;
  }
  : {};

# Returns the names of a `text_data` category in a master database, keyed by ID.
sub read_names( $mastermdb, $category ) {
    my $db = DBI->connect(
        "dbi:SQLite:$mastermdb",
        undef, undef,
        {
            RaiseError         => 1,
            sqlite_open_flags  => SQLITE_OPEN_READONLY,
            sqlite_string_mode => DBD_SQLITE_STRING_MODE_UNICODE_STRICT,
        }
    );
    my $rows = $db->selectall_arrayref(
        <<"SQL", undef, $category
SELECT [index], text
  FROM text_data
  WHERE category = ?;
SQL
    );
    $db->disconnect;
    return { map { $_->[0] => $_->[1] } $rows->@* };
}

# Overwrites the names in the given column (0 = Japanese, 1 = English) of
# every section with those in a master database, except for inherited
# versions of uniques.
sub update_column( $column, $mastermdb ) {
    while ( my ( $section, $category ) = each %categories ) {
        my $dbnames = read_names( $mastermdb, $category );
        while ( my ( $id, $name ) = each $dbnames->%* ) {
            next
              if $section eq 'skills'
              && $id =~ /^9(\d+)/
              && exists $dbnames->{ '1' . $1 };
            $names->{$section}{$id}[$column] = $name;
        }
    }
}

update_column( 0, $jpmdb ) if defined $jpmdb;
update_column( 1, $enmdb ) if defined $enmdb;

# Inherited versions of uniques are always named after their unique plus a
# suffix, as their names in the databases lack it (and may differ slightly
# otherwise, e.g. "Best day ever" for "Best Day Ever").
my $skills   = $names->{skills};
my @suffixes = ( '（継承）', ' (inherited)' );
for my $id ( grep { /^1/ } keys $skills->%* ) {
    my $unique = $skills->{$id};
    for my $column ( grep { length( $unique->[$_] // '' ) } 0, 1 ) {
        $skills->{ $id =~ s/^1/9/r }[$column] =
          $unique->[$column] . $suffixes[$column];
    }
}

# New entries only have the names from the given databases.
for my $pair ( map { values $_->%* } values $names->%* ) {
    $_ //= '' for $pair->@[ 0, 1 ];
}

# One entry per line, so that diffs show which names changed.
sub encode_section($section) {
    my $entries = $names->{$section};
    return
      $json->encode($section) . ":{\n"
      . join( ",\n",
        map { $json->encode("$_") . ':' . $json->encode( $entries->{$_} ) }
        sort { $a <=> $b } keys $entries->%* ) . "\n}";
}

say "{\n", join( ",\n", map { encode_section($_) } sort keys %categories ),
  "\n}";
