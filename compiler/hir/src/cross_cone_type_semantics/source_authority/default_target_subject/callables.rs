use super::*;
use DefaultCallableDeclarationV1 as Declaration;
use DefaultCallableReferenceTargetViewV1 as View;
use DefaultNestedCallableIdentityV1 as Nested;
use ExportDefaultCallableTargetV1 as Callable;
mod identities;

/// Identity-backed access demands. Nested and equality demands require their
/// own full source contracts; none of these variants is an access proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceCallableAccessSubjectV1<'t> {
    Declaration(Subject),
    Nested(Nested),
    DerivedEquality { owner_type: &'t SignatureTypeKey },
}
type Access<'t> = DefaultSourceCallableAccessSubjectV1<'t>;

impl DefaultTargetIdentityQueriesV1<'_> {
    pub fn default_callable_access_subject<'t>(
        &self,
        target: &'t Callable,
        meter: &mut BudgetMeter,
    ) -> Result<Access<'t>, Error> {
        self.default_callable_access_subject_view(target.into(), meter)
    }

    pub(crate) fn default_callable_access_subject_view<'t>(
        &self,
        target: View<'t>,
        meter: &mut BudgetMeter,
    ) -> Result<Access<'t>, Error> {
        let mut query = Query {
            foundation: self,
            meter,
            path: WirePath::root(),
        };
        query.meter.check_semantic_depth(1, &query.path)?;
        query.meter.charge_nodes(1, &query.path)?;
        query.meter.charge_work(1, &query.path)?;
        match target {
            View::Callable(callable) => query.callable(callable.declaration()),
            View::FunctionAddress(declaration) => query.callable(declaration),
            View::Bound(callable) => match callable.source() {
                DefaultBoundCallableSourceV1::Class { callable, .. } => {
                    query.callable(callable.declaration())
                }
                DefaultBoundCallableSourceV1::Interface { member, .. } => {
                    query.callable(function(*member)?)
                }
            },
            View::DerivedEquality(owner_type) => Ok(Access::DerivedEquality { owner_type }),
            View::LocalFunction(declaration) => {
                query.expected_nested(function(declaration)?, Nested::LocalFunction(declaration))
            }
            View::Lambda(body) => {
                query.expected_nested(Declaration::Generated(body), Nested::Lambda(body))
            }
            View::AnonymousFunction(body) => query.expected_nested(
                Declaration::Generated(body),
                Nested::AnonymousFunction(body),
            ),
            View::CallableReference(invoke) => query.expected_nested(
                Declaration::Generated(invoke),
                Nested::CallableReference(invoke),
            ),
        }
    }
}
impl Query<'_, '_, '_> {
    fn expected_nested<'t>(
        &mut self,
        declaration: Declaration,
        expected: Nested,
    ) -> Result<Access<'t>, Error> {
        match self.callable(declaration)? {
            Access::Nested(actual) if actual == expected => Ok(Access::Nested(actual)),
            _ => Err(Error::NestedRole(expected)),
        }
    }
}
fn function(origin: CallableTemplateOrigin) -> Result<Declaration, Error> {
    match origin {
        CallableTemplateOrigin::Function(id) => Ok(Declaration::Function(id)),
        CallableTemplateOrigin::GenericFunction(id) => Ok(Declaration::GenericFunction(id)),
        _ => Err(Error::CallableOrigin(origin)),
    }
}
