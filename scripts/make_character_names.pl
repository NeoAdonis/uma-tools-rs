#!/usr/bin/env perl

use v5.36;
use utf8;
use open qw(:std :encoding(UTF-8));

use DBI;
use DBD::SQLite::Constants qw(:file_open :dbd_sqlite_string_mode);
use Getopt::Long;
use JSON::PP;

my $usage = <<'USAGE';
Usage: make_character_names.pl [--japanese jp.mdb] [--english en.mdb] \
           character_names.json

Updates the Japanese and/or English character names in character_names.json
(if it exists) from the given master databases, overwriting existing names,
and prints the result. At least one database is required.
USAGE

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

# Returns the character names in a master database, keyed by card ID.
sub read_names($mastermdb) {
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
        <<"SQL"
SELECT [index], text
  FROM text_data
  WHERE category = 4;
SQL
    );
    $db->disconnect;
    return { map { $_->[0] => $_->[1] } $rows->@* };
}

# Overwrites the names in the given column (0 = Japanese, 1 = English) with
# those in a master database.
sub update_column( $column, $mastermdb ) {
    my $dbnames = read_names($mastermdb);
    while ( my ( $id, $name ) = each $dbnames->%* ) {
        $names->{$id}[$column] = $name;
    }
}

update_column( 0, $jpmdb ) if defined $jpmdb;
update_column( 1, $enmdb ) if defined $enmdb;

# New characters only have the names from the given databases.
for my $pair ( values $names->%* ) {
    $_ //= '' for $pair->@[ 0, 1 ];
}

# One character per line, so that diffs show which names changed.
say "{\n",
  join( ",\n",
    map  { $json->encode("$_") . ':' . $json->encode( $names->{$_} ) }
    sort { $a <=> $b } keys $names->%* ),
  "\n}";
