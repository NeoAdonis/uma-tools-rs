#!/usr/bin/env perl

use v5.36;

use DBI;
use DBD::SQLite::Constants qw(:file_open);
use Hash::Util             qw(lock_hash);
use JSON::PP;

@ARGV or die "Usage: make_skill_data.pl master.mdb\n";
my ($mastermdb) = @ARGV;

my $db = DBI->connect(
    "dbi:SQLite:$mastermdb",
    undef, undef,
    {
        RaiseError        => 1,
        sqlite_open_flags => SQLITE_OPEN_READONLY
    }
);

my $select = $db->prepare(
    <<"SQL"
SELECT *
  FROM skill_data
  WHERE is_general_skill = 1 OR rarity >= 3;
SQL
);

$select->execute;

my %skills;

# $i is the trigger (1-2), $j is the effect within that trigger (1-3)
sub make_effect( $row, $i, $j ) {
    my $effect = {
        type         => $row->{"ability_type_${i}_${j}"},
        modifier     => $row->{"float_ability_value_${i}_${j}"},
        valueScaling => $row->{"ability_value_usage_${i}_${j}"},
        target       => {
            type  => $row->{"target_type_${i}_${j}"},
            value => $row->{"target_value_${i}_${j}"}
        }
    };
    my $additional = $row->{"additional_activate_type_${i}_${j}"};
    $effect->{additional} = $additional if $additional;
    return $effect;
}

sub make_trigger( $row, $i ) {
    my @effects =
      map { make_effect( $row, $i, $_ ) }
      grep { $_ == 1 || $row->{"ability_type_${i}_$_"} != 0 } 1 .. 3;
    return {
        precondition    => $row->{"precondition_$i"},
        condition       => $row->{"condition_$i"},
        baseDuration    => $row->{"float_ability_time_$i"},
        durationScaling => $row->{"ability_time_usage_$i"},
        baseCooldown    => $row->{"float_cooldown_time_$i"},
        effects         => \@effects
    };
}

while ( my $row = $select->fetchrow_hashref ) {

    # `die` on misspelled column names instead of silently reading undef.
    lock_hash( $row->%* );
    my @triggers =
      map { make_trigger( $row, $_ ) }
      grep {
        my $condition = $row->{"condition_$_"};
        $_ == 1 || ( $condition ne '' && $condition ne '0' )
      } 1 .. 2;
    my @popularity =
      map {
        +{
            stat  => $row->{"popularity_add_param_$_"},
            value => $row->{"popularity_add_value_$_"}
        }
      }
      grep { $row->{"popularity_add_param_$_"} != 0 } 1 .. 2;
    $skills{ $row->{id} } = {
        rarity       => $row->{rarity},
        witCheck  => $row->{activate_lot},
        alternatives => \@triggers,
        popularity   => \@popularity
    };
}

my $json = JSON::PP->new;
$json->canonical(1);
say $json->encode( \%skills );
